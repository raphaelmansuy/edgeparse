#!/usr/bin/env python3
"""Multi-engine PDF benchmark comparison.

Runs EdgeParse against multiple third-party PDF-to-Markdown engines and produces
terminal + HTML reports with side-by-side metrics, charts, and rankings.

Engine groups:
  Non-OCR (fast, no ML models): edgeparse, opendataloader, pymupdf4llm,
                                 markitdown, liteparse
  Hybrid (backend-assisted):    edgeparse, opendataloader_hybrid_docling_fast,
                                 opendataloader_hybrid_hancom
  OCR / ML (model-heavy):       edgeparse, docling, marker, mineru

Usage:
    # Non-OCR comparison (fast, recommended first run):
    uv run python compare_all.py --group non-ocr --install

    # OCR/ML comparison (slow — installs isolated venvs for marker & mineru):
    uv run python compare_all.py --group hybrid

    # OCR/ML comparison (slow — installs isolated venvs for marker & mineru):
    uv run python compare_all.py --group ocr --install

    # Custom engine subset:
    uv run python compare_all.py --engines edgeparse,docling,pymupdf4llm --install

    # All engines (slow):
    uv run python compare_all.py --all --install

    # Reuse existing results (no re-run):
    uv run python compare_all.py --group non-ocr --no-run

    # List engine availability:
    uv run python compare_all.py --list

Via Makefile:
    make bench-non-ocr
    make bench-hybrid
    make bench-ocr
    make bench-ocr OCR_ENGINES=docling
    make bench-compare-all
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import logging
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import Request, urlopen
from typing import Dict, List, Optional, Sequence

# Add src to path
sys.path.insert(0, str(Path(__file__).parent / "src"))

from engine_registry import (
    ENGINES, ENGINE_META, NON_OCR_ENGINES, HYBRID_ENGINES, OCR_ENGINES,
    available_engines, display_name,
)
from evaluation_schema import missing_evaluation_requirements
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

# All known engines in preferred display order
ALL_ENGINES = [
    # Non-OCR (fast)
    "edgeparse", "opendataloader", "pymupdf4llm", "markitdown", "liteparse",
    # Hybrid
    "opendataloader_hybrid_docling_fast", "opendataloader_hybrid_hancom",
    # OCR / ML
    "chandra", "docling", "marker", "mineru",
]

# pip install commands for each engine
INSTALL_COMMANDS = {
    "opendataloader": "opendataloader-pdf>=2.0.0",
    "pymupdf4llm":    "pymupdf4llm",
    "markitdown":     "markitdown[all]",
    "liteparse":      "@llamaindex/liteparse",  # installed via run.py node adapter
    "chandra":        "chandra-ocr",
    "docling":        "docling",
    "marker":         "marker-pdf",
    "mineru":         "mineru[all]",
}

# Engines that are not installed via pip/uv and require manual setup.
MANUAL_INSTALL_HINTS = {
    "chandra": "pip install chandra-ocr  (then run `chandra_vllm` or set CHANDRA_METHOD=hf and install chandra-ocr[hf])",
    "opendataloader_hybrid_docling_fast": "pip install opendataloader-pdf[hybrid] and run opendataloader-pdf-hybrid",
    "opendataloader_hybrid_hancom": "configure Hancom backend and run the hybrid service",
}


def _load_result(engine: str) -> Optional[dict]:
    """Load evaluation JSON for the given engine, or None if not found."""
    path = PREDICTION_DIR / engine / "evaluation.json"
    if not path.exists():
        return None
    with path.open(encoding="utf-8") as f:
        data = json.load(f)

    missing = missing_evaluation_requirements(data)
    if missing:
        print(
            f"  {YELLOW}⚠ Stale evaluation for {display_name(engine)}; "
            f"refreshing metrics-only output{RESET}"
        )
        cmd = [
            sys.executable,
            str(BENCH_DIR / "run.py"),
            "--engine",
            engine,
            "--skip-parse",
            "--log-level",
            "WARNING",
        ]
        result = subprocess.run(cmd, cwd=str(BENCH_DIR))
        if result.returncode == 0:
            with path.open(encoding="utf-8") as f:
                data = json.load(f)
        else:
            print(
                f"  {RED}✗ Failed to refresh {display_name(engine)}; "
                f"missing {', '.join(missing[:4])}"
                f"{' ...' if len(missing) > 4 else ''}{RESET}"
            )
    return data


def _run_engine(engine: str, max_docs: Optional[int] = None) -> bool:
    """Run benchmark for a single engine. Returns True on success."""
    cmd = [sys.executable, str(BENCH_DIR / "run.py"), "--engine", engine]
    if max_docs:
        cmd.extend(["--max-docs", str(max_docs)])
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
    "chandra":     "chandra",
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

def _tcp_reachable(host: str, port: int, timeout_s: float = 0.4) -> bool:
    try:
        with socket.create_connection((host, port), timeout=timeout_s):
            return True
    except OSError:
        return False

def _openai_models_endpoint_ok(api_base: str, timeout_s: float = 0.8) -> bool:
    """Return True if the given base URL looks like an OpenAI-compatible endpoint."""
    url = api_base.rstrip("/") + "/models"
    try:
        req = Request(url, headers={"Accept": "application/json"})
        with urlopen(req, timeout=timeout_s) as resp:
            if getattr(resp, "status", 200) != 200:
                return False
            body = resp.read(2048)
    except Exception:
        return False
    # Cheap check to avoid importing json in this hot path.
    return b"data" in body


def _check_engine_available(engine: str) -> bool:
    """Check if an engine is installed and available."""
    if engine == "edgeparse":
        binary = BENCH_DIR.parent / "target" / "release" / "edgeparse"
        return binary.exists()
    if engine == "opendataloader":
        # opendataloader-pdf ships a CLI command. It lives in the active venv's
        # bin/ during `uv run`, so shutil.which covers both venv and system PATH.
        return shutil.which("opendataloader-pdf") is not None
    if engine == "chandra":
        # Chandra is a CLI-based OCR engine.
        if shutil.which("chandra") is None:
            return False
        method = os.environ.get("CHANDRA_METHOD", "vllm").lower()
        if method == "hf":
            # HF mode requires torch.
            return importlib.util.find_spec("torch") is not None
        # vLLM mode requires a running local server (default: http://localhost:8000/v1).
        api_base = os.environ.get("VLLM_API_BASE", "http://localhost:8000/v1")
        parsed = urlparse(api_base)
        host = parsed.hostname or "localhost"
        port = parsed.port or (443 if parsed.scheme == "https" else 80)
        return _tcp_reachable(host, port) and _openai_models_endpoint_ok(api_base)
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


def _run_engine_isolated(engine: str, max_docs: Optional[int] = None) -> bool:
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
    if max_docs:
        cmd.extend(["--max-docs", str(max_docs)])
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
    title: str = "EdgeParse Multi-Engine Benchmark",
    max_docs: Optional[int] = None,
) -> Dict[str, dict]:
    """Run benchmarks for all specified engines and produce reports.

    Returns dict of engine_name → evaluation data.
    """
    # ── Header ────────────────────────────────────────────────────────────────
    print()
    print(f"╔{'═' * 68}╗")
    print(f"║  {BOLD}{title}{RESET}{'':>{68 - 4 - len(title)}}║")
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
                # Some engines require additional runtime setup (e.g. a local server)
                # beyond package installation. Only schedule if they are now runnable.
                if _check_engine_available(eng):
                    active_engines.append(eng)
                else:
                    print(f"  {YELLOW}⚠ Skipping {display_name(eng)} (runtime not ready){RESET}")
                    hint = MANUAL_INSTALL_HINTS.get(eng)
                    if hint:
                        print(f"    {DIM}Setup: {hint}{RESET}")
            else:
                print(f"  {YELLOW}⚠ Skipping {display_name(eng)} (install failed){RESET}")
        else:
            print(f"  {YELLOW}⚠ Skipping {display_name(eng)} (not installed){RESET}")
            hint = MANUAL_INSTALL_HINTS.get(eng)
            if hint:
                print(f"    {DIM}Install with: {hint}{RESET}")
            else:
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
            success = (
                _run_engine_isolated(eng, max_docs=max_docs)
                if is_isolated
                else _run_engine(eng, max_docs=max_docs)
            )
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
        shutil.copy2(str(html_output), str(latest))
    print(f"  {DIM}Latest: {latest}{RESET}")

    # ── Save comparison JSON ─────────────────────────────────────────────────
    comparison_json = html_output.with_suffix(".json")
    summary = {}
    for eng, data in results.items():
        scores = data.get("metrics", {}).get("score", {})
        td = data.get("table_detection", {})
        spd = data.get("speed", {})
        summary[eng] = {
            "display_name": display_name(eng),
            # Structural metrics
            "nid": scores.get("nid_mean"),
            "teds": scores.get("teds_mean"),
            "mhs": scores.get("mhs_mean"),
            "paragraph_boundary_f1": scores.get("paragraph_boundary_f1_mean"),
            # Text content quality metrics
            "text_quality_score": scores.get("text_quality_score_mean"),
            "rouge1": scores.get("rouge1_mean"),
            "rouge2": scores.get("rouge2_mean"),
            "rougeL": scores.get("rougeL_mean"),
            "bleu4": scores.get("bleu4_mean"),
            "word_fragmentation_score": scores.get("word_fragmentation_score_mean"),
            "f1_token": scores.get("f1_token_mean"),
            "cer": scores.get("cer_mean"),
            "wer": scores.get("wer_mean"),
            # Composite + auxiliary
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
  uv run python compare_all.py --group non-ocr --install
  uv run python compare_all.py --group hybrid
  uv run python compare_all.py --group ocr --install
  uv run python compare_all.py --engines edgeparse,docling,pymupdf4llm --install
  uv run python compare_all.py --all --no-run
  uv run python compare_all.py --list
        """,
    )
    parser.add_argument(
        "--group",
        choices=["non-ocr", "hybrid", "ocr", "all"],
        default=None,
        help="Engine group to benchmark: non-ocr (fast), hybrid (backend-assisted), ocr (ML/model-heavy), all",
    )
    parser.add_argument(
        "--engines",
        type=str,
        default=None,
        help="Comma-separated list of engines (overrides --group). E.g. edgeparse,docling,pymupdf4llm",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Run all known engines (non-OCR + OCR). Equivalent to --group all.",
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
        "--title",
        type=str,
        default=None,
        help="Custom title for the HTML report header",
    )
    parser.add_argument(
        "--max-docs",
        type=int,
        default=None,
        help="Limit all engines to the first N documents (by ground-truth order). Useful for expensive OCR/VLM engines.",
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

    # ── Determine engine list ──────────────────────────────────────────────────
    # Priority: --engines > --group > --all > default (installed only)
    if args.engines:
        engines = [e.strip() for e in args.engines.split(",") if e.strip()]
        default_title = "EdgeParse Benchmark — Custom Selection"
    elif args.all or args.group == "all":
        engines = ALL_ENGINES
        default_title = "EdgeParse Multi-Engine Benchmark — All Engines"
    elif args.group == "non-ocr":
        engines = list(NON_OCR_ENGINES)
        default_title = "EdgeParse Benchmark — Non-OCR Tools"
    elif args.group == "hybrid":
        engines = list(HYBRID_ENGINES)
        default_title = "EdgeParse Benchmark — Hybrid Tools"
    elif args.group == "ocr":
        engines = list(OCR_ENGINES)
        default_title = "EdgeParse Benchmark — OCR / ML Tools"
    else:
        # Default: edgeparse + whatever else is installed (non-OCR only)
        engines = ["edgeparse"] + [
            e for e in NON_OCR_ENGINES[1:] if _check_engine_available(e)
        ]
        default_title = "EdgeParse Benchmark"

    title = args.title or default_title
    html_path = Path(args.html) if args.html else None

    run_comparison(
        engines=engines,
        skip_run=args.no_run,
        install_missing=args.install,
        html_output=html_path,
        title=title,
        max_docs=args.max_docs,
    )


if __name__ == "__main__":
    main()
