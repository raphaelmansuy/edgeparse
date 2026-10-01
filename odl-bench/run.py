#!/usr/bin/env python3
"""Official OpenDataLoader board harness runner.

Scores overall = mean(NID, TEDS, MHS). Separate from benchmark/.
"""

from __future__ import annotations

import argparse
import json
import logging
import sys
from pathlib import Path
from typing import List, Optional

sys.path.insert(0, str(Path(__file__).parent / "src"))

from evaluator import (  # noqa: E402
    DEFAULT_GT_DIR,
    DEFAULT_OUTPUT_FILENAME,
    DEFAULT_PREDICTION_ROOT,
    run as evaluate_run,
)
from engine_registry import available_engines, display_name  # noqa: E402
from pdf_parser import DEFAULT_INPUT_DIR, process_markdown  # noqa: E402


def _resolve_path(value: str, project_root: Path) -> Path:
    path = Path(value)
    return path if path.is_absolute() else project_root / path


def run_harness(args: argparse.Namespace) -> dict:
    project_root = Path(__file__).parent.resolve()
    input_dir = _resolve_path(args.input_dir, project_root)
    ground_truth_dir = _resolve_path(args.ground_truth_dir, project_root)
    prediction_root = _resolve_path(args.prediction_root, project_root)

    engine_name = args.engine or "edgeparse"

    doc_ids: Optional[List[str]] = None
    if args.doc_id:
        doc_ids = None
    elif getattr(args, "max_docs", None):
        max_docs = int(args.max_docs)
        if max_docs > 0:
            gt_paths = sorted(ground_truth_dir.glob("*.md"))
            doc_ids = [p.stem for p in gt_paths[:max_docs]]

    if args.skip_parse:
        logging.info("Skipping PDF parsing for %s; refreshing evaluation only.", engine_name)
    else:
        logging.info("Starting PDF parsing with %s (%s)...", engine_name, display_name(engine_name))
        process_markdown(engine_name, str(input_dir), doc_id=args.doc_id, doc_ids=doc_ids)

    logging.info("Running official-board evaluation (mean of NID, TEDS, MHS)...")
    evaluation_paths = evaluate_run(
        str(ground_truth_dir),
        str(prediction_root),
        args.evaluation_filename,
        target_engine=engine_name,
        target_doc_id=args.doc_id,
        target_doc_ids=doc_ids,
    )

    result: dict = {"engine": engine_name, "evaluation_paths": [str(p) for p in evaluation_paths]}
    if evaluation_paths:
        payload = json.loads(evaluation_paths[0].read_text(encoding="utf-8"))
        score = payload.get("metrics", {}).get("score", {})
        result["score"] = score
        _print_score_row(engine_name, score, payload.get("summary", {}))
    return result


def _print_score_row(engine: str, score: dict, summary: dict) -> None:
    def fmt(key: str) -> str:
        v = score.get(key)
        return f"{v:.3f}" if isinstance(v, (int, float)) else "  —  "

    secs = summary.get("elapsed_per_page")
    if secs is None and summary.get("elapsed_per_doc") is not None:
        secs = summary["elapsed_per_doc"]
    speed = f"{secs:.3f}" if isinstance(secs, (int, float)) else "  —  "

    print()
    print(f"{'Engine':<28} {'Overall':>8} {'NID':>8} {'TEDS':>8} {'MHS':>8} {'s/page':>8}")
    print("-" * 72)
    print(
        f"{engine:<28} {fmt('overall_mean'):>8} {fmt('nid_mean'):>8} "
        f"{fmt('teds_mean'):>8} {fmt('mhs_mean'):>8} {speed:>8}"
    )
    print()


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Official ODL board harness (overall = mean of NID, TEDS, MHS)"
    )
    parser.add_argument("--engine", type=str, default="edgeparse",
                        help=f"Engine to run. Available: {', '.join(available_engines())}")
    parser.add_argument("--input-dir", type=str, default=DEFAULT_INPUT_DIR)
    parser.add_argument("--ground-truth-dir", type=str, default=DEFAULT_GT_DIR)
    parser.add_argument("--prediction-root", type=str, default=DEFAULT_PREDICTION_ROOT)
    parser.add_argument("--evaluation-filename", type=str, default=DEFAULT_OUTPUT_FILENAME)
    parser.add_argument("--doc-id", type=str, default=None)
    parser.add_argument("--max-docs", type=int, default=None)
    parser.add_argument("--skip-parse", action="store_true")
    parser.add_argument("--list-engines", action="store_true")
    parser.add_argument("--log-level", type=str, default="INFO")
    args = parser.parse_args(argv)

    logging.basicConfig(
        level=getattr(logging, args.log_level.upper(), logging.INFO),
        format="%(levelname)s %(message)s",
    )

    if args.list_engines:
        print("Available engines:")
        for name in available_engines():
            print(f"  {name:30s}  {display_name(name)}")
        return 0

    run_harness(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
