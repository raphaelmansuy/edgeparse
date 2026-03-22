#!/usr/bin/env python3
"""EdgeParse vs OpenDataLoader — side-by-side benchmark comparison.

Usage
-----
    # Run both engines, then show comparison:
    uv run python compare.py

    # Reuse existing results (no re-run):
    uv run python compare.py --no-run

    # Run only one engine, reuse the other:
    uv run python compare.py --skip-edgeparse
    uv run python compare.py --skip-odl

Via Makefile:
    make bench-compare          # full run + compare
    make bench-compare-report   # compare from existing results
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Optional, Sequence

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


# ══════════════════════════════════════════════════════════════════════════════
#  Data loading
# ══════════════════════════════════════════════════════════════════════════════

def _load_result(engine: str) -> Optional[dict]:
    """Load evaluation JSON for the given engine, or None if not found."""
    path = PREDICTION_DIR / engine / "evaluation.json"
    if not path.exists():
        return None
    with path.open(encoding="utf-8") as f:
        return json.load(f)


# ══════════════════════════════════════════════════════════════════════════════
#  Benchmark runners
# ══════════════════════════════════════════════════════════════════════════════

def _run_engine(engine: str) -> None:
    """Invoke run.py for the given engine in a subprocess."""
    import subprocess
    cmd = [sys.executable, str(BENCH_DIR / "run.py"), "--engine", engine]
    result = subprocess.run(cmd, cwd=str(BENCH_DIR))
    if result.returncode != 0:
        print(f"{RED}Benchmark failed for engine: {engine}{RESET}", file=sys.stderr)
        raise SystemExit(result.returncode)


# ══════════════════════════════════════════════════════════════════════════════
#  Formatting helpers
# ══════════════════════════════════════════════════════════════════════════════

def _fmt(value: Optional[float], precision: int = 4) -> str:
    if value is None:
        return "N/A"
    return f"{value:.{precision}f}"


def _delta_str(a: Optional[float], b: Optional[float], higher_better: bool = True) -> str:
    """Return formatted delta  (a − b)  with colour and direction arrow."""
    if a is None or b is None:
        return "N/A"
    delta = a - b
    positive_is_good = delta > 0 if higher_better else delta < 0
    colour = GREEN if positive_is_good else (RED if delta != 0 else DIM)
    arrow = "▲" if delta > 0 else ("▼" if delta < 0 else "=")
    sign = "+" if delta > 0 else ""
    return f"{colour}{sign}{delta:.4f} {arrow}{RESET}"


def _speed_ratio(edgeparse_spd: Optional[float], odl_spd: Optional[float]) -> str:
    """Return a human-readable speed ratio string."""
    if edgeparse_spd is None or odl_spd is None or edgeparse_spd == 0:
        return "N/A"
    if edgeparse_spd < odl_spd:
        ratio = odl_spd / edgeparse_spd
        return f"{GREEN}{ratio:.1f}× faster{RESET}"
    elif edgeparse_spd > odl_spd:
        ratio = edgeparse_spd / odl_spd
        return f"{RED}{ratio:.1f}× slower{RESET}"
    return f"{DIM}same speed{RESET}"


def _winner_label(a: Optional[float], b: Optional[float], higher_better: bool = True) -> str:
    if a is None or b is None or a == b:
        return f"{DIM}Tie{RESET}"
    wins = (a > b) if higher_better else (a < b)
    return f"{GREEN}EdgeParse{RESET}" if wins else f"{CYAN}OpenDataLoader{RESET}"


def _speed_winner(edgeparse_spd: Optional[float], odl_spd: Optional[float]) -> str:
    if edgeparse_spd is None or odl_spd is None or edgeparse_spd == odl_spd:
        return f"{DIM}Tie{RESET}"
    return (
        f"{GREEN}EdgeParse{RESET}" if edgeparse_spd < odl_spd
        else f"{CYAN}OpenDataLoader{RESET}"
    )


# ══════════════════════════════════════════════════════════════════════════════
#  Report rendering
# ══════════════════════════════════════════════════════════════════════════════

SEP = "─" * 78
BOX_TOP    = "╔" + "═" * 76 + "╗"
BOX_BOTTOM = "╚" + "═" * 76 + "╝"


def _box_line(text: str) -> str:
    # Strip ANSI for width calculation, then pad
    import re
    plain = re.sub(r"\033\[[0-9;]*m", "", text)
    padding = 76 - len(plain)
    return f"║  {text}{' ' * max(0, padding - 2)}║"


def _header(edgeparse_data: Optional[dict], odl_data: Optional[dict]) -> None:
    # Pick processor and date from whichever result is available
    data = edgeparse_data or odl_data
    processor = ""
    run_date = time.strftime("%b %d, %Y")
    doc_count = 0
    if data:
        spd = data.get("speed", {})
        processor = spd.get("processor", "")
        doc_count = spd.get("document_count", 0)

    print()
    print(BOX_TOP)
    title = f"{BOLD}EdgeParse (Rust)  vs  OpenDataLoader (Java){RESET} — Benchmark Report"
    print(_box_line(title))
    meta_parts = []
    if doc_count:
        meta_parts.append(f"{doc_count} PDFs")
    meta_parts.append(run_date)
    if processor:
        meta_parts.append(processor)
    meta = f"{DIM}{'  ·  '.join(meta_parts)}{RESET}"
    print(_box_line(meta))
    print(BOX_BOTTOM)
    print()


def _metrics_table(edgeparse_data: Optional[dict], odl_data: Optional[dict]) -> None:
    e_scores   = (edgeparse_data or {}).get("metrics", {}).get("score", {})
    o_scores   = (odl_data or {}).get("metrics", {}).get("score", {})
    e_td       = (edgeparse_data or {}).get("table_detection", {})
    o_td       = (odl_data or {}).get("table_detection", {})
    e_spd      = (edgeparse_data or {}).get("speed", {})
    o_spd      = (odl_data or {}).get("speed", {})

    e_nid  = e_scores.get("nid_mean")
    o_nid  = o_scores.get("nid_mean")
    e_teds = e_scores.get("teds_mean")
    o_teds = o_scores.get("teds_mean")
    e_mhs  = e_scores.get("mhs_mean")
    o_mhs  = o_scores.get("mhs_mean")
    e_f1   = e_td.get("f1")
    o_f1   = o_td.get("f1")
    e_ep   = e_spd.get("elapsed_per_doc")
    o_ep   = o_spd.get("elapsed_per_doc")

    # ── Accuracy table ────────────────────────────────────────────────────────
    col = [28, 12, 14, 20, 18]
    hdr = (
        f"{'Metric':<{col[0]}}"
        f"{'EdgeParse':>{col[1]}}"
        f"{'OpenDataLoader':>{col[2]}}"
        f"{'Δ (EdgeParse − ODL)':>{col[3]}}"
        f"{'Winner':>{col[4]}}"
    )
    print(f"{BOLD}{hdr}{RESET}")
    print(SEP)

    rows = [
        ("NID  (Reading Order)",  e_nid,  o_nid,  True),
        ("TEDS (Tables)",         e_teds, o_teds, True),
        ("MHS  (Headings)",       e_mhs,  o_mhs,  True),
        ("Table Detection F1",    e_f1,   o_f1,   True),
    ]
    edgeparse_wins = 0
    odl_wins = 0
    for label, ev, ov, hb in rows:
        delta = _delta_str(ev, ov, hb)
        winner = _winner_label(ev, ov, hb)
        if ev is not None and ov is not None and ev != ov:
            if (ev > ov) == hb:
                edgeparse_wins += 1
            else:
                odl_wins += 1
        print(
            f"{label:<{col[0]}}"
            f"{_fmt(ev):>{col[1]}}"
            f"{_fmt(ov):>{col[2]}}"
            f"  {delta:<38}"
            f"  {winner}"
        )

    # Speed row (lower is better)
    spd_ratio = _speed_ratio(e_ep, o_ep)
    spd_win   = _speed_winner(e_ep, o_ep)
    if e_ep is not None and o_ep is not None and e_ep != o_ep:
        if e_ep < o_ep:
            edgeparse_wins += 1
        else:
            odl_wins += 1
    spd_delta = _delta_str(e_ep, o_ep, higher_better=False)
    print(
        f"{'Speed (s/doc)':<{col[0]}}"
        f"{_fmt(e_ep, precision=3):>{col[1]}}"
        f"{_fmt(o_ep, precision=3):>{col[2]}}"
        f"  {spd_delta:<38}"
        f"  {spd_win}  ({spd_ratio})"
    )

    print(SEP)

    # ── Table detection detail ─────────────────────────────────────────────────
    print()
    print(f"{BOLD}Table Detection Detail{RESET}")
    print(SEP)
    detail_col = [20, 10, 14]
    det_hdr = (
        f"{'Metric':<{detail_col[0]}}"
        f"{'EdgeParse':>{detail_col[1]}}"
        f"{'OpenDataLoader':>{detail_col[2]}}"
    )
    print(f"{DIM}{det_hdr}{RESET}")

    det_rows = [
        ("Precision",   e_td.get("precision"), o_td.get("precision")),
        ("Recall",      e_td.get("recall"),    o_td.get("recall")),
        ("F1",          e_td.get("f1"),         o_td.get("f1")),
        ("Accuracy",    e_td.get("accuracy"),   o_td.get("accuracy")),
    ]
    for label, ev, ov in det_rows:
        print(
            f"{label:<{detail_col[0]}}"
            f"{_fmt(ev):>{detail_col[1]}}"
            f"{_fmt(ov):>{detail_col[2]}}"
        )

    # Confusion matrix
    e_tp = e_td.get("tp", 0)
    e_fp = e_td.get("fp", 0)
    e_fn = e_td.get("fn", 0)
    e_tn = e_td.get("tn", 0)
    o_tp = o_td.get("tp", 0)
    o_fp = o_td.get("fp", 0)
    o_fn = o_td.get("fn", 0)
    o_tn = o_td.get("tn", 0)
    print()
    print(
        f"{'Confusion Matrix':<{detail_col[0]}}"
        f"{'EdgeParse':>{detail_col[1]}}"
        f"{'OpenDataLoader':>{detail_col[2]}}"
    )
    print(
        f"  {'TP/FP':<{detail_col[0] - 2}}"
        f"{f'{e_tp}/{e_fp}':>{detail_col[1]}}"
        f"{f'{o_tp}/{o_fp}':>{detail_col[2]}}"
    )
    print(
        f"  {'FN/TN':<{detail_col[0] - 2}}"
        f"{f'{e_fn}/{e_tn}':>{detail_col[1]}}"
        f"{f'{o_fn}/{o_tn}':>{detail_col[2]}}"
    )

    # ── Verdict ────────────────────────────────────────────────────────────────
    print()
    print(SEP)
    total = edgeparse_wins + odl_wins
    if total == 0:
        verdict = f"{DIM}No results available — run benchmarks first.{RESET}"
    elif edgeparse_wins > odl_wins:
        verdict = (
            f"{BOLD}{GREEN}EdgeParse{RESET}{BOLD} wins "
            f"{edgeparse_wins}/{total} metrics.{RESET}  "
            f"{DIM}OpenDataLoader wins {odl_wins}/{total}.{RESET}"
        )
    elif odl_wins > edgeparse_wins:
        verdict = (
            f"{BOLD}{CYAN}OpenDataLoader{RESET}{BOLD} wins "
            f"{odl_wins}/{total} metrics.{RESET}  "
            f"{DIM}EdgeParse wins {edgeparse_wins}/{total}.{RESET}"
        )
    else:
        verdict = (
            f"{BOLD}{YELLOW}Tie{RESET}{BOLD}: each engine wins "
            f"{edgeparse_wins}/{total} metrics.{RESET}"
        )
    print(f"  {BOLD}Verdict:{RESET}  {verdict}")
    print(SEP)
    print()


# ══════════════════════════════════════════════════════════════════════════════
#  CLI
# ══════════════════════════════════════════════════════════════════════════════

def _parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="EdgeParse vs OpenDataLoader — side-by-side benchmark comparison"
    )
    parser.add_argument(
        "--no-run",
        action="store_true",
        help="Skip running benchmarks; load existing prediction results",
    )
    parser.add_argument(
        "--skip-edgeparse",
        action="store_true",
        help="Skip the edgeparse benchmark run (reuse existing results)",
    )
    parser.add_argument(
        "--skip-odl",
        action="store_true",
        help="Skip the opendataloader benchmark run (reuse existing results)",
    )
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> None:
    args = _parse_args(argv)

    run_edgeparse = not args.no_run and not args.skip_edgeparse
    run_odl     = not args.no_run and not args.skip_odl

    if run_edgeparse:
        print(f"{BOLD} ▶{RESET} Running edgeparse benchmark ...")
        _run_engine("edgeparse")

    if run_odl:
        print(f"{BOLD} ▶{RESET} Running opendataloader benchmark ...")
        _run_engine("opendataloader")

    edgeparse_data = _load_result("edgeparse")
    odl_data     = _load_result("opendataloader")

    if edgeparse_data is None and odl_data is None:
        print(
            f"{RED}No benchmark results found.\n"
            f"Run: make bench-compare{RESET}",
            file=sys.stderr,
        )
        raise SystemExit(1)

    if edgeparse_data is None:
        print(f"{YELLOW}Warning: no EdgeParse results found in {PREDICTION_DIR}/edgeparse/{RESET}")
    if odl_data is None:
        print(
            f"{YELLOW}Warning: no OpenDataLoader results found in "
            f"{PREDICTION_DIR}/opendataloader/\n"
            f"Install with: make bench-odl-setup  then  make bench-odl{RESET}"
        )

    _header(edgeparse_data, odl_data)
    _metrics_table(edgeparse_data, odl_data)


if __name__ == "__main__":
    main()
