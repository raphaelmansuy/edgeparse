#!/usr/bin/env python3
"""Report the metric gaps between EdgeParse and current board leaders.

Reads a multi-engine comparison JSON produced by benchmark/compare_all.py and
prints a compact decision-oriented report for OODA iterations.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict, Iterable, Optional, Tuple


METRICS = {
    "nid": {"higher_better": True, "label": "NID"},
    "teds": {"higher_better": True, "label": "TEDS"},
    "mhs": {"higher_better": True, "label": "MHS"},
    "paragraph_boundary_f1": {"higher_better": True, "label": "PBF"},
    "text_quality_score": {"higher_better": True, "label": "TQS"},
    "table_detection_f1": {"higher_better": True, "label": "TD F1"},
    "speed_per_doc": {"higher_better": False, "label": "Speed"},
    "overall": {"higher_better": True, "label": "Overall"},
}


def _load_report(path: Path) -> Dict[str, dict]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def _leader(
    report: Dict[str, dict], metric: str, higher_better: bool
) -> Tuple[Optional[str], Optional[float]]:
    candidates = []
    for engine, payload in report.items():
        value = payload.get(metric)
        if value is None:
            continue
        candidates.append((engine, value))
    if not candidates:
        return None, None
    candidates.sort(key=lambda item: item[1], reverse=higher_better)
    return candidates[0]


def _gap(target: float, current: float, higher_better: bool) -> float:
    if higher_better:
        return target - current
    return current - target


def _format(value: Optional[float], metric: str) -> str:
    if value is None:
        return "N/A"
    if metric == "speed_per_doc":
        return f"{value:.3f}s"
    return f"{value:.4f}"


def main() -> int:
    parser = argparse.ArgumentParser(description="Compute EdgeParse gaps to board leaders.")
    parser.add_argument(
        "report",
        nargs="?",
        default="reports/benchmark-20260325-145420.json",
        help="Path to a multi-engine comparison JSON report",
    )
    parser.add_argument(
        "--engine",
        default="edgeparse",
        help="Engine name to compare against the board leaders",
    )
    args = parser.parse_args()

    report_path = Path(args.report)
    report = _load_report(report_path)
    if args.engine not in report:
        raise SystemExit(f"Engine '{args.engine}' not found in {report_path}")

    current = report[args.engine]

    print(f"Report: {report_path}")
    print(f"Focus engine: {args.engine}")
    print()
    print(f"{'Metric':<10} {'Current':>10} {'Leader':>12} {'Leader val':>12} {'Gap':>12} {'Status':>10}")
    print("-" * 72)

    open_gaps = []
    wins = []

    for metric, info in METRICS.items():
        leader_engine, leader_value = _leader(report, metric, info["higher_better"])
        current_value = current.get(metric)
        if current_value is None or leader_value is None:
            status = "N/A"
            gap_value = None
        else:
            gap_value = _gap(leader_value, current_value, info["higher_better"])
            if gap_value <= 0:
                status = "WIN"
                wins.append(metric)
            else:
                status = "GAP"
                open_gaps.append((metric, gap_value, leader_engine, leader_value, current_value))

        print(
            f"{info['label']:<10} "
            f"{_format(current_value, metric):>10} "
            f"{(leader_engine or 'N/A'):>12} "
            f"{_format(leader_value, metric):>12} "
            f"{('N/A' if gap_value is None else f'{gap_value:.4f}'):>12} "
            f"{status:>10}"
        )

    print()
    if open_gaps:
        open_gaps.sort(key=lambda row: row[1], reverse=True)
        print("Open gaps ranked by raw metric distance:")
        for metric, gap_value, leader_engine, leader_value, current_value in open_gaps:
            label = METRICS[metric]["label"]
            print(
                f"- {label}: {gap_value:.4f} behind {leader_engine} "
                f"({_format(current_value, metric)} vs {_format(leader_value, metric)})"
            )
    else:
        print("No open gaps. Focus on defending the lead and preserving speed.")

    print()
    print(f"Wins: {len(wins)}/{len(METRICS)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())