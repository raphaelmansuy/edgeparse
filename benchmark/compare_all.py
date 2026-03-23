#!/usr/bin/env python3
"""Multi-engine PDF benchmark comparison.

Runs EdgeParse against multiple third-party PDF-to-Markdown engines and produces
terminal + HTML reports with side-by-side metrics, charts, and rankings.

Supported engines: edgeparse, opendataloader, docling, marker, mineru,
                   pymupdf4llm, markitdown

Usage:
    # Compare EdgeParse with lightweight tools (fast):
    uv run python compare_all.py --engines edgeparse,pymupdf4llm,markitdown

    # Compare all installed engines (skip missing):
    uv run python compare_all.py --all

    # Reuse existing results:
    uv run python compare_all.py --all --no-run

    # Install missing engines before running:
    uv run python compare_all.py --engines edgeparse,docling,pymupdf4llm --install

Via Makefile:
    make bench-compare-all
    make bench-compare-all ENGINES=edgeparse,pymupdf4llm,markitdown
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import logging
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Sequence

# Add src to path
sys.path.insert(0, str(Path(__file__).parent / "src"))

from engine_registry import ENGINES, ENGINE_META, available_engines, display_name
from report_terminal import print_comparison_report, print_single_report
from report_html import generate_html_report

# ── ANSI colours ─────────────────────────────────────────────────────────────
BOLD   = "\033[1m"
GREEN  = "\033[0;32m"
CYAN   = "\033[0;36m"
YELLOW = "\033[0;33m"
RED    = "\033[0;31m"
DIM    = "\033[2m"
RESET  = "\033[0m"

BENCH_DIR = Path(__file__).parent.resolve()
PREDICTION_DIR = BENCH_DIR / "prediction"
# Dedicated per-engine venvs for packages that conflict with the base environment
# (marker-pdf and mineru[all] each carry incompatible torch/torchvision).
ISOLATED_VENVS_DIR = BENCH_DIR / ".venvs"
# Dedicated per-engine venvs for packages that conflict with the base environment
# (e.g. marker-pdf and mineru[all] each carry incompatible torch/torchvision).
ISOLATED_VENVS_DIR = BENCH_DIR / ".venvs"

# All known engines in preferred display order (EdgeQuake removed)
ALL_ENGINES = ["edgeparse", "opendataloader", "docling", "pymupdf4llm", "markitdown", "liteparse"]

# pip install commands for each engine
INSTALL_COMMANDS = {
    "opendataloader": "opendataloader-pdf>=2.0.0",
    "docling":        "docling",
    "marker":         "marker-pdf",
    "mineru":         "mineru[all]",
    "pymupdf4llm":    "pymupdf4llm",
    "markitdown":     "markitdown[all]",
    # edgequake-pdf2md is a Rust binary installed via cargo, not pip
}


def _load_result(engine: str) -> Optional[dict]:
    """Load evaluation JSON for the given engine, or None if not found."""
    path = PREDICTION_DIR / engine / "evaluation.json"
    if not path.exists():
        return None
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def _run_engine(engine: str) -> bool:
    """Run benchmark for a single engine. Returns True on success."""
    cmd = [sys.executable, str(BENCH_DIR / "run.py"), "--engine", engine]
    result = subprocess.run(cmd, cwd=str(BENCH_DIR))
    return result.returncode == 0


def _install_engine(engine: str) -> bool:
    """Attempt to install a missing engine. Returns True on success."""
    pkg = INSTALL_COMMANDS.get(engine)
    if not pkg:
        return False

    print(f"  {CYAN}Installing {display_name(engine)} ({pkg})...{RESET}")
    # uv-managed venvs don't bundle pip as a Python module; prefer `uv pip install`.
    # Fall back to `python -m pip install` for non-uv environments.
    uv_bin = shutil.which("uv")
    if uv_bin:
        cmd = [uv_bin, "pip", "install", "--upgrade", pkg]
    else:
        cmd = [sys.executable, "-m", "pip", "install", "--upgrade", pkg]
    result = subprocess.run(cmd, capture_output=True, text=True, cwd=str(BENCH_DIR))
    if result.returncode == 0:
        print(f"  {GREEN}✓ {display_name(engine)} installed successfully{RESET}")
        return True
    else:
        print(f"  {RED}✗ Failed to install {display_name(engine)}: {result.stderr[:200]}{RESET}")
        return False


# Map engine names → actual Python import module to verify installation.
# Parser adapter files (pdf_parser_*.py) use lazy imports, so "engine in ENGINES"
# is not sufficient — the adapter always imports cleanly even if the pip
# package is absent.  We probe the actual package directly.
_ENGINE_PKG_MODULE: Dict[str, str] = {
    "pymupdf4llm": "pymupdf4llm",
    "markitdown":  "markitdown",
    "docling":     "docling",
    "marker":      "marker",
    "mineru":      "mineru",
}

# Engines whose pip packages have irreconcilable dependency conflicts (pillow)
# and cannot be installed together in one venv.  These are run via
# `uv run --with <pkg> python run.py --engine <name>` in an isolated environment.
_ISOLATED_ENGINES: Dict[str, str] = {
    "marker": "marker-pdf",
    "mineru": "mineru[all]",
}


def _check_engine_available(engine: str) -> bool:
    """Check if an engine is installed and available."""
    if engine == "edgeparse":
        binary = BENCH_DIR.parent / "target" / "release" / "edgeparse"
        return binary.exists()
    if engine == "opendataloader":
        # opendataloader-pdf ships a CLI command. It lives in the active venv's
        # bin/ during `uv run`, so shutil.which covers both venv and system PATH.
        return shutil.which("opendataloader-pdf") is not None
    if engine == "edgequake":
        # pdf2md is a standalone Rust binary installed via cargo
        cargo_bin = Path.home() / ".cargo" / "bin" / "pdf2md"
        return cargo_bin.exists() or shutil.which("pdf2md") is not None
    # Isolated engines are never permanently installed – always run via uv --with.
    if engine in _ISOLATED_ENGINES:
        return False
    pkg_mod = _ENGINE_PKG_MODULE.get(engine)
    if pkg_mod:
        return importlib.util.find_spec(pkg_mod) is not None
    return engine in ENGINES


def _get_or_create_isolated_venv(engine: str) -> Optional[Path]:
    """Return the Python interpreter for the engine's dedicated isolated venv.

    Creates the venv and installs the package if it doesn't exist yet.
    Returns None on failure.
    """
    pkg = _ISOLATED_ENGINES.get(engine)
    if not pkg:
        return None

    uv_bin = shutil.which("uv")
    if not uv_bin:
        print(f"  {RED}✗ uv not found; cannot create isolated venv for {display_name(engine)}{RESET}")
        return None

    venv_dir = ISOLATED_VENVS_DIR / engine
    python_bin = venv_dir / "bin" / "python"

    if not python_bin.exists():
        dname = display_name(engine)
        print(f"  {CYAN}⊕ Creating isolated venv for {dname}...{RESET}")
        r = subprocess.run([uv_bin, "venv", str(venv_dir)],
                           capture_output=True, text=True)
        if r.returncode != 0:
            print(f"  {RED}✗ venv creation failed: {r.stderr[:200]}{RESET}")
            return None

        print(f"  {CYAN}⊕ Installing {dname} ({pkg}) into isolated venv...{RESET}")
        r = subprocess.run(
            [uv_bin, "pip", "install", "--python", str(python_bin), pkg],
            capture_output=True, text=True,
        )
        if r.returncode != 0:
            print(f"  {RED}✗ Install failed: {r.stderr[:400]}{RESET}")
            return None
        print(f"  {GREEN}✓ {dname} ready in isolated venv{RESET}")

    return python_bin


def _run_engine_isolated(engine: str) -> bool:
    """Run the benchmark for an engine in its own dedicated isolated venv.

    Marker and MinerU carry torch/pillow versions that conflict with the base
    benchmark venv.  Running them in a separate venv avoids those conflicts.

    PYTHONPATH is set so the isolated interpreter can import:
      • benchmark/src   — our evaluators, parsers, report modules
      • base venv site-packages — apted, rapidfuzz, bs4, lxml, cpuinfo, etc.
    The isolated venv's own site-packages take precedence (Python always loads
    the active venv's packages before PYTHONPATH entries).
    """
    python_bin = _get_or_create_isolated_venv(engine)
    if not python_bin:
        return False

    # Collect paths the isolated interpreter needs to reach.
    extra_paths = [str(BENCH_DIR / "src")]
    # Add base benchmark venv site-packages so evaluators can import apted etc.
    base_site = BENCH_DIR / ".venv" / "lib"
    if base_site.exists():
        for py_dir in base_site.iterdir():
            sp = py_dir / "site-packages"
            if sp.exists():
                extra_paths.append(str(sp))
                break

    env = os.environ.copy()
    existing = env.get("PYTHONPATH", "")
    env["PYTHONPATH"] = ":".join(extra_paths + ([existing] if existing else []))

    cmd = [str(python_bin), str(BENCH_DIR / "run.py"), "--engine", engine]
    result = subprocess.run(cmd, cwd=str(BENCH_DIR), env=env)
    return result.returncode == 0


def print_engine_status():
    """Print the availability status of all known engines."""
    print()
    print(f"  {BOLD}Engine Availability{RESET}")
    print(f"  {'─' * 60}")
    for eng in ALL_ENGINES:
        meta = ENGINE_META.get(eng, (eng, None, ""))
        dname = meta[0]
        pkg = meta[1] or "built-in"
        if eng in _ISOLATED_ENGINES:
            venv_python = ISOLATED_VENVS_DIR / eng / "bin" / "python"
            if venv_python.exists():
                status = f"{GREEN}✓ isolated venv{RESET}"
            else:
                status = f"{CYAN}~ isolated (venv pending){RESET}"
        elif _check_engine_available(eng):
            status = f"{GREEN}✓ installed{RESET}"
        else:
            status = f"{DIM}✗ not installed{RESET}"
        print(f"  {dname:<16} {status:<30} {DIM}({pkg}){RESET}")
    print()


def run_comparison(
    engines: List[str],
    skip_run: bool = False,
    install_missing: bool = False,
    html_output: Optional[Path] = None,
) -> Dict[str, dict]:
    """Run benchmarks for all specified engines and produce reports.

    Returns dict of engine_name → evaluation data.
    """
    # ── Header ────────────────────────────────────────────────────────────────
    print()
    print(f"╔{'═' * 68}╗")
    print(f"║  {BOLD}EdgeParse Multi-Engine Benchmark{RESET}{'':40}║")
    print(f"║  {DIM}Methodology: opendataloader.org/docs/benchmark{RESET}{'':21}║")
    print(f"╚{'═' * 68}╝")
    print()

    # ── Check availability, install if requested, plan isolated engines ───────
    active_engines: List[str] = []    # run via normal _run_engine()
    isolated_engines: List[str] = []  # run via uv run --with (conflict-safe)

    for eng in engines:
        if skip_run:
            # When only loading existing results, skip availability checks entirely
            if eng in _ISOLATED_ENGINES:
                isolated_engines.append(eng)
            else:
                active_engines.append(eng)
        elif _check_engine_available(eng):
            active_engines.append(eng)
        elif eng in _ISOLATED_ENGINES:
            # Always schedule isolated engines — uv --with handles the install
            isolated_engines.append(eng)
        elif install_missing:
            if _install_engine(eng):
                active_engines.append(eng)
            else:
                print(f"  {YELLOW}⚠ Skipping {display_name(eng)} (install failed){RESET}")
        else:
            print(f"  {YELLOW}⚠ Skipping {display_name(eng)} (not installed){RESET}")
            pkg = INSTALL_COMMANDS.get(eng, "?")
            print(f"    {DIM}Install with: pip install {pkg}{RESET}")

    all_planned = active_engines + isolated_engines
    if not all_planned:
        print(f"\n{RED}No engines available. Install at least one engine.{RESET}")
        raise SystemExit(1)

    print(f"\n  {BOLD}Engines to benchmark:{RESET} {', '.join(display_name(e) for e in all_planned)}")
    if isolated_engines:
        print(f"  {DIM}(isolated via uv --with: {', '.join(display_name(e) for e in isolated_engines)}){RESET}")
    print()

    # ── Run benchmarks ────────────────────────────────────────────────────────
    results: Dict[str, dict] = {}
    total_start = time.time()

    for i, eng in enumerate(all_planned, 1):
        dname = display_name(eng)
        is_isolated = eng in isolated_engines
        if skip_run:
            print(f"  [{i}/{len(all_planned)}] {CYAN}Loading existing results for {dname}...{RESET}")
        else:
            print(f"  [{i}/{len(all_planned)}] {BOLD}▶ Running {dname} benchmark...{RESET}")
            start = time.time()
            success = _run_engine_isolated(eng) if is_isolated else _run_engine(eng)
            elapsed = time.time() - start
            if success:
                print(f"  {GREEN}✓ {dname} completed in {elapsed:.1f}s{RESET}")
            else:
                print(f"  {RED}✗ {dname} benchmark failed{RESET}")
                continue

        data = _load_result(eng)
        if data:
            results[eng] = data
        else:
            print(f"  {YELLOW}⚠ No results found for {dname}{RESET}")

    total_elapsed = time.time() - total_start

    if not results:
        print(f"\n{RED}No benchmark results available.{RESET}")
        raise SystemExit(1)

    print(f"\n  {DIM}Total benchmark time: {total_elapsed:.1f}s{RESET}")

    # ── Terminal Report ──────────────────────────────────────────────────────
    if len(results) == 1:
        eng = list(results.keys())[0]
        print_single_report(results[eng], eng)
    else:
        print_comparison_report(results)

    # ── HTML Report ──────────────────────────────────────────────────────────
    if html_output is None:
        html_output = BENCH_DIR / "reports" / f"benchmark-{time.strftime('%Y%m%d-%H%M%S')}.html"
    html_output.parent.mkdir(parents=True, exist_ok=True)

    generate_html_report(results, html_output)
    print(f"  {GREEN}✓ HTML report saved:{RESET} {html_output}")

    # Also save a "latest" symlink/copy
    latest = html_output.parent / "benchmark-latest.html"
    try:
        if latest.exists() or latest.is_symlink():
            latest.unlink()
        latest.symlink_to(html_output.name)
    except OSError:
        # Fallback: just copy
        import shutil
        shutil.copy2(str(html_output), str(latest))
    print(f"  {DIM}Latest report: {latest}{RESET}")

    # ── Save comparison JSON ─────────────────────────────────────────────────
    comparison_json = html_output.with_suffix(".json")
    summary = {}
    for eng, data in results.items():
        scores = data.get("metrics", {}).get("score", {})
        td = data.get("table_detection", {})
        spd = data.get("speed", {})
        summary[eng] = {
            "display_name": display_name(eng),
            "nid": scores.get("nid_mean"),
            "teds": scores.get("teds_mean"),
            "mhs": scores.get("mhs_mean"),
            "paragraph_boundary_f1": scores.get("paragraph_boundary_f1_mean"),
            "overall": scores.get("overall_mean"),
            "table_detection_f1": td.get("f1"),
            "speed_per_doc": spd.get("elapsed_per_doc"),
            "document_count": spd.get("document_count"),
        }
    comparison_json.write_text(json.dumps(summary, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"  {DIM}JSON summary: {comparison_json}{RESET}")
    print()

    return results


def _parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Multi-engine PDF-to-Markdown benchmark comparison",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  uv run python compare_all.py --engines edgeparse,pymupdf4llm,markitdown
  uv run python compare_all.py --all --install
  uv run python compare_all.py --all --no-run
  uv run python compare_all.py --list
        """,
    )
    parser.add_argument(
        "--engines",
        type=str,
        default=None,
        help="Comma-separated list of engines to compare (e.g. edgeparse,docling,pymupdf4llm)",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Run all available engines",
    )
    parser.add_argument(
        "--no-run",
        action="store_true",
        help="Skip running benchmarks; load existing results only",
    )
    parser.add_argument(
        "--install",
        action="store_true",
        help="Attempt to install missing engines before running",
    )
    parser.add_argument(
        "--html",
        type=str,
        default=None,
        help="Output path for HTML report (default: benchmark/reports/benchmark-<timestamp>.html)",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        dest="list_engines",
        help="List available engines and exit",
    )
    parser.add_argument(
        "--log-level",
        default="INFO",
        help="Logging verbosity (e.g. INFO, DEBUG)",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> None:
    args = _parse_args(argv)
    logging.basicConfig(
        level=getattr(logging, args.log_level.upper(), logging.INFO),
        format="%(asctime)s - %(levelname)s - %(message)s",
    )

    if args.list_engines:
        print_engine_status()
        return

    # Determine engines to run
    if args.engines:
        engines = [e.strip() for e in args.engines.split(",") if e.strip()]
    elif args.all:
        engines = ALL_ENGINES
    else:
        # Default: edgeparse + whatever else is installed
        engines = ["edgeparse"] + [e for e in ALL_ENGINES[1:] if _check_engine_available(e)]

    html_path = Path(args.html) if args.html else None

    run_comparison(
        engines=engines,
        skip_run=args.no_run,
        install_missing=args.install,
        html_output=html_path,
    )


if __name__ == "__main__":
    main()
