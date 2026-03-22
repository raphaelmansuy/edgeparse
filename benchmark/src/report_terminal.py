"""Beautiful terminal output for benchmark results.

Provides rich, colour-coded terminal reporting with metric explanations
inspired by the opendataloader.org benchmark presentation style.
"""

from __future__ import annotations

import re
import time
from typing import Dict, List, Optional, Any

# ── ANSI colours ─────────────────────────────────────────────────────────────
BOLD   = "\033[1m"
GREEN  = "\033[0;32m"
CYAN   = "\033[0;36m"
YELLOW = "\033[0;33m"
RED    = "\033[0;31m"
DIM    = "\033[2m"
BLUE   = "\033[0;34m"
MAGENTA = "\033[0;35m"
WHITE  = "\033[1;37m"
BG_GREEN  = "\033[42m"
BG_RED    = "\033[41m"
BG_YELLOW = "\033[43m"
BG_BLUE   = "\033[44m"
RESET  = "\033[0m"
UNDERLINE = "\033[4m"

# ── Box drawing ──────────────────────────────────────────────────────────────
SEP        = "─" * 80
SEP_DOUBLE = "═" * 80
BOX_TOP    = "╔" + "═" * 78 + "╗"
BOX_BOTTOM = "╚" + "═" * 78 + "╝"
BOX_SEP    = "╟" + "─" * 78 + "╢"

# ── Metric definitions ───────────────────────────────────────────────────────
METRIC_INFO = {
    "nid": {
        "name": "NID — Reading Order",
        "short": "Reading Order",
        "unit": "[0–1]",
        "higher_better": True,
        "description": (
            "Normalized Indel Distance: measures whether text is extracted in "
            "the correct sequence. A score of 1.0 means perfect reading order. "
            "Critical for multi-column layouts, sidebars, and academic papers."
        ),
    },
    "teds": {
        "name": "TEDS — Table Structure",
        "short": "Table Structure",
        "unit": "[0–1]",
        "higher_better": True,
        "description": (
            "Tree Edit Distance Similarity: compares extracted table structure "
            "against ground truth. A score of 1.0 means perfect table reconstruction. "
            "Critical for financial docs, technical specs, and comparison tables."
        ),
    },
    "mhs": {
        "name": "MHS — Heading Hierarchy",
        "short": "Heading Level",
        "unit": "[0–1]",
        "higher_better": True,
        "description": (
            "Markdown Heading Similarity: compares detected headings and their "
            "levels against ground truth. A score of 1.0 means all headings are "
            "correctly identified with proper hierarchy (h1 > h2 > h3). "
            "Essential for semantic chunking in RAG pipelines."
        ),
    },
    "paragraph_boundary_f1": {
        "name": "PBF — Paragraph Boundaries",
        "short": "Paragraphs",
        "unit": "[0–1]",
        "higher_better": True,
        "description": (
            "Paragraph Boundary F1: compares paragraph breaks against ground truth. "
            "It penalizes merged paragraphs and spurious splits even when token "
            "content is otherwise similar."
        ),
    },
    "prose_block_boundary_f1": {
        "name": "SBF — Prose Block Boundaries",
        "short": "Prose Blocks",
        "unit": "[0–1]",
        "higher_better": True,
        "description": (
            "Structure Boundary F1: compares paragraph-like prose blocks, "
            "including list and caption blocks, against ground truth. It catches "
            "structural misses that paragraph-only scoring can under-report."
        ),
    },
    "table_detection_f1": {
        "name": "Table Detection F1",
        "short": "Table Det. F1",
        "unit": "[0–1]",
        "higher_better": True,
        "description": (
            "F1 score for detecting whether a page contains a table. "
            "Combines precision (no false alarms) and recall (no missed tables)."
        ),
    },
    "speed": {
        "name": "Speed",
        "short": "Speed",
        "unit": "s/doc",
        "higher_better": False,
        "description": (
            "Average seconds per document across the benchmark corpus. "
            "Covers the full pipeline: PDF parsing, layout analysis, and "
            "Markdown generation. Lower is better. Measured single-threaded on CPU."
        ),
    },
}


def _strip_ansi(text: str) -> str:
    return re.sub(r"\033\[[0-9;]*m", "", text)


def _pad_right(text: str, width: int) -> str:
    """Right-pad text accounting for ANSI escape codes."""
    visible_len = len(_strip_ansi(text))
    return text + " " * max(0, width - visible_len)


def _box_line(text: str, width: int = 78) -> str:
    """Wrap text in a box line ║ ... ║."""
    plain_len = len(_strip_ansi(text))
    padding = width - plain_len - 2
    return f"║ {text}{' ' * max(0, padding)} ║"


def _score_bar(value: float, width: int = 20, color: str = GREEN) -> str:
    """Generate a horizontal bar for a score value [0–1]."""
    filled = int(value * width)
    empty = width - filled
    return f"{color}{'█' * filled}{DIM}{'░' * empty}{RESET}"


def _score_color(value: Optional[float], metric_key: str) -> str:
    """Color a score based on quality thresholds."""
    if value is None:
        return f"{DIM}  N/A  {RESET}"
    if metric_key == "speed":
        if value < 0.5:
            color = GREEN
        elif value < 2.0:
            color = YELLOW
        else:
            color = RED
    else:
        if value >= 0.85:
            color = GREEN
        elif value >= 0.60:
            color = YELLOW
        else:
            color = RED
    return f"{color}{value:.4f}{RESET}"


def _rank_badge(rank: int, total: int) -> str:
    """Return a coloured rank badge."""
    if rank == 1:
        return f"{BG_GREEN}{BOLD} #1 {RESET}"
    elif rank == 2:
        return f"{GREEN} #2 {RESET}"
    elif rank == 3:
        return f"{YELLOW} #3 {RESET}"
    elif rank == total:
        return f"{DIM} #{rank} {RESET}"
    else:
        return f"{DIM} #{rank} {RESET}"


# ══════════════════════════════════════════════════════════════════════════════
#  Single-engine report
# ══════════════════════════════════════════════════════════════════════════════

def print_single_report(eval_data: dict, engine_name: str = "edgeparse") -> None:
    """Print a beautiful single-engine benchmark report."""
    scores = eval_data.get("metrics", {}).get("score", {})
    table_detection = eval_data.get("table_detection", {})
    speed = eval_data.get("speed", {})
    triage = eval_data.get("triage", {})

    nid = scores.get("nid_mean")
    teds = scores.get("teds_mean")
    mhs = scores.get("mhs_mean")
    paragraph_boundary_f1 = scores.get("paragraph_boundary_f1_mean")
    prose_block_boundary_f1 = scores.get("prose_block_boundary_f1_mean")
    td_f1 = table_detection.get("f1")
    elapsed_per_doc = speed.get("elapsed_per_doc")
    total_elapsed = speed.get("total_elapsed")
    document_count = speed.get("document_count")
    processor = speed.get("processor", "")

    from engine_registry import display_name
    disp_name = display_name(engine_name)

    # Header
    print()
    print(BOX_TOP)
    title = f"{BOLD}{disp_name} Benchmark Report{RESET}"
    print(_box_line(title))
    meta_parts = []
    if document_count:
        meta_parts.append(f"{document_count} documents")
    meta_parts.append(time.strftime("%Y-%m-%d %H:%M"))
    if processor:
        meta_parts.append(processor)
    print(_box_line(f"{DIM}{'  ·  '.join(meta_parts)}{RESET}"))
    print(BOX_BOTTOM)
    print()

    # Metric explanations header
    print(f"  {BOLD}{UNDERLINE}What We Measure{RESET}  {DIM}(source: opendataloader.org/docs/benchmark){RESET}")
    print()

    # Score cards
    metrics = [
        ("nid",  nid),
        ("teds", teds),
        ("mhs",  mhs),
        ("paragraph_boundary_f1", paragraph_boundary_f1),
        ("prose_block_boundary_f1", prose_block_boundary_f1),
        ("table_detection_f1", td_f1),
    ]
    for key, value in metrics:
        info = METRIC_INFO[key]
        score_str = _score_color(value, key)
        bar = _score_bar(value, 25) if value is not None else ""
        print(f"  {BOLD}{info['name']:<30}{RESET} {score_str}  {bar}")
        print(f"  {DIM}{info['description'][:100]}{RESET}")
        print()

    # Speed
    spd_info = METRIC_INFO["speed"]
    if elapsed_per_doc is not None:
        spd_color = GREEN if elapsed_per_doc < 0.5 else (YELLOW if elapsed_per_doc < 2.0 else RED)
        print(f"  {BOLD}{spd_info['name']:<30}{RESET} {spd_color}{elapsed_per_doc:.3f} s/doc{RESET}")
        if total_elapsed is not None:
            print(f"  {DIM}Total: {total_elapsed:.1f}s for {document_count} documents{RESET}")
    else:
        print(f"  {BOLD}{spd_info['name']:<30}{RESET} {DIM}N/A{RESET}")
    print(f"  {DIM}{spd_info['description'][:100]}{RESET}")
    print()

    # Table Detection Detail
    if table_detection:
        print(f"  {BOLD}Table Detection Confusion Matrix{RESET}")
        tp = table_detection.get("tp", 0)
        fp = table_detection.get("fp", 0)
        fn = table_detection.get("fn", 0)
        tn = table_detection.get("tn", 0)
        print(f"    {GREEN}TP: {tp:3d}{RESET}  │  {RED}FP: {fp:3d}{RESET}")
        print(f"    {RED}FN: {fn:3d}{RESET}  │  {GREEN}TN: {tn:3d}{RESET}")
        td_prec = table_detection.get("precision")
        td_rec = table_detection.get("recall")
        td_acc = table_detection.get("accuracy")
        if td_prec is not None:
            print(f"    Precision: {td_prec:.4f}   Recall: {td_rec:.4f}   Accuracy: {td_acc:.4f}")
        print()

    # Triage
    if triage and triage.get("total_pages_evaluated", 0) > 0:
        print(f"  {BOLD}Triage (Hybrid Mode){RESET}")
        tr = triage
        print(f"    Recall: {_score_color(tr.get('recall'), 'nid')}   "
              f"Precision: {_score_color(tr.get('precision'), 'nid')}   "
              f"F1: {_score_color(tr.get('f1'), 'nid')}")
        print()

    # Overall score
    overall = scores.get("overall_mean")
    if overall is not None:
        if overall >= 0.85:
            grade_color = GREEN
            grade = "A"
        elif overall >= 0.70:
            grade_color = YELLOW
            grade = "B"
        elif overall >= 0.50:
            grade_color = YELLOW
            grade = "C"
        else:
            grade_color = RED
            grade = "D"
        print(SEP)
        print(f"  {BOLD}Overall Score:{RESET}  {grade_color}{BOLD}{overall:.4f}{RESET}  "
              f"({grade_color}{grade}{RESET})")
        print(SEP)
    print()


# ══════════════════════════════════════════════════════════════════════════════
#  Multi-engine comparison report
# ══════════════════════════════════════════════════════════════════════════════

def print_comparison_report(results: Dict[str, dict]) -> None:
    """Print a side-by-side comparison of multiple engines."""
    from engine_registry import display_name

    if not results:
        print(f"{RED}No benchmark results to compare.{RESET}")
        return

    engines = list(results.keys())
    n = len(engines)

    # Header
    print()
    print(BOX_TOP)
    title = f"{BOLD}PDF-to-Markdown Benchmark Comparison{RESET}"
    print(_box_line(title))
    subtitle = f"{DIM}{n} engines · {time.strftime('%Y-%m-%d %H:%M')} · opendataloader.org methodology{RESET}"
    print(_box_line(subtitle))
    print(BOX_BOTTOM)
    print()

    # Metric explanations
    print(f"  {BOLD}{UNDERLINE}Metrics Explained{RESET}")
    print()
    for key in ["nid", "teds", "mhs", "paragraph_boundary_f1", "speed"]:
        info = METRIC_INFO[key]
        direction = f"{GREEN}↑ higher is better{RESET}" if info["higher_better"] else f"{CYAN}↓ lower is better{RESET}"
        print(f"  {BOLD}{info['short']:.<25}{RESET} {direction}")
        print(f"  {DIM}{info['description'][:110]}{RESET}")
        print()

    # ── Quick Comparison Table ────────────────────────────────────────────────
    print(SEP_DOUBLE)
    print(f"  {BOLD}Quick Comparison{RESET}  {DIM}Scores normalized to [0–1]. Higher is better for accuracy; lower for speed.{RESET}")
    print(SEP)

    # Column widths
    name_w = max(len(display_name(e)) for e in engines) + 2
    col_w = 10

    # Header row
    header = f"  {'Engine':<{name_w}} {'NID':>{col_w}} {'TEDS':>{col_w}} {'MHS':>{col_w}} {'PBF':>{col_w}} {'TD F1':>{col_w}} {'s/doc':>{col_w}} {'Overall':>{col_w}}"
    print(f"{BOLD}{header}{RESET}")
    print(SEP)

    # Collect values for ranking
    metric_values: Dict[str, List] = {
        "nid": [], "teds": [], "mhs": [], "pbf": [], "td_f1": [], "speed": [], "overall": []
    }
    for eng in engines:
        d = results[eng]
        scores = d.get("metrics", {}).get("score", {})
        td = d.get("table_detection", {})
        spd = d.get("speed", {})
        metric_values["nid"].append((eng, scores.get("nid_mean")))
        metric_values["teds"].append((eng, scores.get("teds_mean")))
        metric_values["mhs"].append((eng, scores.get("mhs_mean")))
        metric_values["pbf"].append((eng, scores.get("paragraph_boundary_f1_mean")))
        metric_values["td_f1"].append((eng, td.get("f1")))
        metric_values["speed"].append((eng, spd.get("elapsed_per_doc")))
        metric_values["overall"].append((eng, scores.get("overall_mean")))

    # Compute ranks
    def _rank(values: list, higher_better: bool = True) -> Dict[str, int]:
        scored = [(e, v) for e, v in values if v is not None]
        scored.sort(key=lambda x: x[1], reverse=higher_better)
        return {e: i + 1 for i, (e, _) in enumerate(scored)}

    ranks = {
        "nid": _rank(metric_values["nid"], True),
        "teds": _rank(metric_values["teds"], True),
        "mhs": _rank(metric_values["mhs"], True),
        "pbf": _rank(metric_values["pbf"], True),
        "td_f1": _rank(metric_values["td_f1"], True),
        "speed": _rank(metric_values["speed"], False),
        "overall": _rank(metric_values["overall"], True),
    }

    # Print rows
    for eng in engines:
        d = results[eng]
        scores = d.get("metrics", {}).get("score", {})
        td = d.get("table_detection", {})
        spd = d.get("speed", {})

        nid = scores.get("nid_mean")
        teds = scores.get("teds_mean")
        mhs = scores.get("mhs_mean")
        pbf = scores.get("paragraph_boundary_f1_mean")
        f1 = td.get("f1")
        ep = spd.get("elapsed_per_doc")
        overall = scores.get("overall_mean")

        def _cell(val, key):
            if val is None:
                return f"{DIM}{'N/A':>{col_w}}{RESET}"
            r = ranks[key].get(eng)
            if r == 1:
                return f"{GREEN}{BOLD}{val:>{col_w}.4f}{RESET}"
            elif r == 2:
                return f"{CYAN}{val:>{col_w}.4f}{RESET}"
            else:
                return f"{val:>{col_w}.4f}"

        def _speed_cell(val):
            if val is None:
                return f"{DIM}{'N/A':>{col_w}}{RESET}"
            r = ranks["speed"].get(eng)
            if r == 1:
                return f"{GREEN}{BOLD}{val:>{col_w}.3f}{RESET}"
            elif r == 2:
                return f"{CYAN}{val:>{col_w}.3f}{RESET}"
            else:
                return f"{val:>{col_w}.3f}"

        dname = display_name(eng)
        row = (f"  {dname:<{name_w}} "
               f"{_cell(nid, 'nid')} "
               f"{_cell(teds, 'teds')} "
               f"{_cell(mhs, 'mhs')} "
               f"{_cell(pbf, 'pbf')} "
               f"{_cell(f1, 'td_f1')} "
               f"{_speed_cell(ep)} "
               f"{_cell(overall, 'overall')}")
        print(row)

    print(SEP)

    # ── Visual bar chart (inline) ─────────────────────────────────────────────
    print()
    print(f"  {BOLD}Visual Comparison{RESET}")
    print()

    for metric_key, label in [("nid", "NID (Reading Order)"), ("teds", "TEDS (Tables)"),
                               ("mhs", "MHS (Headings)"), ("pbf", "PBF (Paragraph Boundaries)")]:
        print(f"  {BOLD}{label}{RESET}")
        entries = metric_values[metric_key]
        entries_sorted = sorted(
            [(e, v) for e, v in entries if v is not None],
            key=lambda x: x[1], reverse=True,
        )
        for eng, val in entries_sorted:
            dname = display_name(eng)
            bar = _score_bar(val, 30)
            r = ranks[metric_key].get(eng, 0)
            badge = _rank_badge(r, n)
            print(f"    {dname:<16} {bar} {val:.4f} {badge}")
        print()

    # Speed chart (lower is better, scale differently)
    print(f"  {BOLD}Speed (s/doc) — lower is better{RESET}")
    speed_entries = sorted(
        [(e, v) for e, v in metric_values["speed"] if v is not None],
        key=lambda x: x[1],
    )
    if speed_entries:
        max_speed = max(v for _, v in speed_entries)
        for eng, val in speed_entries:
            dname = display_name(eng)
            bar_len = int((val / max(max_speed, 0.001)) * 30)
            bar = f"{CYAN}{'█' * bar_len}{DIM}{'░' * (30 - bar_len)}{RESET}"
            r = ranks["speed"].get(eng, 0)
            badge = _rank_badge(r, n)
            print(f"    {dname:<16} {bar} {val:.3f}s {badge}")
    print()

    # ── Verdict ───────────────────────────────────────────────────────────────
    print(SEP_DOUBLE)
    # Count wins per engine
    win_counts: Dict[str, int] = {e: 0 for e in engines}
    for metric_key in ["nid", "teds", "mhs", "td_f1", "speed"]:
        for eng, rank in ranks[metric_key].items():
            if rank == 1:
                win_counts[eng] += 1

    winner = max(win_counts, key=win_counts.get)
    winner_wins = win_counts[winner]
    total_metrics = 5

    print(f"  {BOLD}Verdict:{RESET}  {GREEN}{BOLD}{display_name(winner)}{RESET}"
          f" wins {winner_wins}/{total_metrics} metrics.")

    # Runner up
    others = [(e, c) for e, c in win_counts.items() if e != winner and c > 0]
    if others:
        others.sort(key=lambda x: x[1], reverse=True)
        parts = [f"{display_name(e)}: {c}" for e, c in others]
        print(f"  {DIM}Other wins: {', '.join(parts)}{RESET}")

    print(SEP_DOUBLE)
    print()
