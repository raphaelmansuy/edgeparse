#!/usr/bin/env python3
"""Build the official-board report from odl-bench prediction evaluations.

Writes ``reports/board.json`` and prints a terminal table:
Overall, NID, TEDS, MHS, s/page.
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(Path(__file__).parent / "src"))

from engine_registry import (  # noqa: E402
    BOARD_ENGINES,
    SKIPPED_ENGINES,
    display_name,
)


def _load_evaluation(engine_dir: Path) -> Optional[dict]:
    path = engine_dir / "evaluation.json"
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def collect_board(prediction_root: Path) -> Dict[str, Any]:
    rows: List[Dict[str, Any]] = []

    for engine in BOARD_ENGINES:
        engine_dir = prediction_root / engine
        payload = _load_evaluation(engine_dir) if engine_dir.is_dir() else None
        if payload is None:
            rows.append(
                {
                    "engine": engine,
                    "display_name": display_name(engine),
                    "status": "not_run",
                    "finished_all": False,
                    "score": {},
                    "summary": {},
                }
            )
            continue

        score = payload.get("metrics", {}).get("score", {})
        summary = payload.get("summary", {}) or {}
        metrics = payload.get("metrics", {})
        doc_count = metrics.get("document_count") or len(payload.get("documents", []))
        missing = metrics.get("missing_predictions", 0)
        finished_all = doc_count >= 200 and missing == 0

        secs = summary.get("elapsed_per_page")
        if secs is None:
            secs = summary.get("elapsed_per_doc")

        rows.append(
            {
                "engine": engine,
                "display_name": display_name(engine),
                "status": "finished" if finished_all else "partial",
                "finished_all": finished_all,
                "document_count": doc_count,
                "missing_predictions": missing,
                "score": {
                    "overall_mean": score.get("overall_mean"),
                    "nid_mean": score.get("nid_mean"),
                    "teds_mean": score.get("teds_mean"),
                    "mhs_mean": score.get("mhs_mean"),
                    "nid_s_mean": score.get("nid_s_mean"),
                    "teds_s_mean": score.get("teds_s_mean"),
                    "mhs_s_mean": score.get("mhs_s_mean"),
                },
                "seconds_per_page": secs,
                "summary": summary,
            }
        )

    skipped = [
        {"engine": name, "reason": reason, "status": "skipped"}
        for name, reason in SKIPPED_ENGINES.items()
    ]

    finished = [r for r in rows if r.get("finished_all") and r["score"].get("overall_mean") is not None]
    finished_sorted = sorted(
        finished, key=lambda r: r["score"]["overall_mean"], reverse=True
    )
    leader = finished_sorted[0] if finished_sorted else None

    edgeparse_rows = [
        r for r in rows if r["engine"] in {"edgeparse", "edgeparse_hybrid"} and r.get("finished_all")
    ]
    beats_all = False
    if edgeparse_rows and finished_sorted:
        best_ep = max(edgeparse_rows, key=lambda r: r["score"]["overall_mean"] or 0)
        competitors = [
            r
            for r in finished_sorted
            if r["engine"] not in {"edgeparse", "edgeparse_hybrid"}
        ]
        if competitors:
            best_comp = competitors[0]["score"]["overall_mean"]
            beats_all = (best_ep["score"]["overall_mean"] or 0) > best_comp
        else:
            beats_all = True

    return {
        "formula": "overall = mean(nid, teds, mhs) with nulls omitted",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "leader": leader["engine"] if leader else None,
        "edgeparse_beats_finished_competitors": beats_all,
        "engines": rows,
        "skipped": skipped,
    }


def print_table(board: Dict[str, Any]) -> None:
    print()
    print("Official OpenDataLoader board (odl-bench)")
    print("overall = mean(NID, TEDS, MHS)")
    print()
    header = f"{'Engine':<32} {'Status':<10} {'Overall':>8} {'NID':>8} {'TEDS':>8} {'MHS':>8} {'s/page':>8}"
    print(header)
    print("-" * len(header))

    def fmt(v: Any) -> str:
        return f"{v:.3f}" if isinstance(v, (int, float)) else "  —  "

    for row in board["engines"]:
        score = row.get("score") or {}
        print(
            f"{row['display_name']:<32} {row['status']:<10} "
            f"{fmt(score.get('overall_mean')):>8} "
            f"{fmt(score.get('nid_mean')):>8} "
            f"{fmt(score.get('teds_mean')):>8} "
            f"{fmt(score.get('mhs_mean')):>8} "
            f"{fmt(row.get('seconds_per_page')):>8}"
        )

    if board.get("skipped"):
        print()
        print("Not run:")
        for item in board["skipped"]:
            print(f"  {item['engine']}: {item['reason']}")

    print()
    if board.get("leader"):
        print(f"Leader (finished 200): {board['leader']}")
    print(
        f"EdgeParse beats finished competitors: "
        f"{board.get('edgeparse_beats_finished_competitors')}"
    )
    print()


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Compare odl-bench engine scores")
    parser.add_argument(
        "--prediction-root",
        type=str,
        default="prediction",
        help="Directory containing engine prediction outputs",
    )
    parser.add_argument(
        "--output",
        type=str,
        default="reports/board.json",
        help="Board JSON output path",
    )
    args = parser.parse_args(argv)

    project_root = Path(__file__).parent.resolve()
    prediction_root = Path(args.prediction_root)
    if not prediction_root.is_absolute():
        prediction_root = project_root / prediction_root

    board = collect_board(prediction_root)
    print_table(board)

    output_path = Path(args.output)
    if not output_path.is_absolute():
        output_path = project_root / output_path
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(board, indent=2, ensure_ascii=False) + "\n")
    print(f"Wrote {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
