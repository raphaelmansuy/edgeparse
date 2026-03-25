"""End-to-end Markdown evaluator.

This module walks through prediction outputs, runs the individual
evaluation routines (heading level, reading order, and table similarity),
and emits a consolidated JSON report that combines the runtime summary
metadata with the computed scores.

The script can be executed directly. By default it evaluates every engine
version found under ``prediction`` and stores ``evaluation.json`` next to
the corresponding ``summary`` file.
"""

from __future__ import annotations

import argparse
import csv
import json
import logging
import time
from dataclasses import dataclass
from pathlib import Path
from statistics import fmean
from typing import Any, Dict, Iterable, List, Optional, Set

from evaluation_schema import CURRENT_EVALUATION_SCHEMA_VERSION
from evaluator_heading_level import evaluate_heading_level
from evaluator_paragraph import evaluate_paragraph_structure
from evaluator_reading_order import evaluate_reading_order
from evaluator_table import evaluate_table
from evaluator_text_quality import evaluate_text_quality


DEFAULT_GT_DIR = "ground-truth/markdown"
DEFAULT_PREDICTION_ROOT = "prediction"
DEFAULT_OUTPUT_FILENAME = "evaluation.json"


@dataclass
class DocumentScores:
    """Container for per-document evaluation results."""

    document_id: str
    overall: Optional[float]
    nid: Optional[float]
    nid_s: Optional[float]
    teds: Optional[float]
    teds_s: Optional[float]
    mhs: Optional[float]
    mhs_s: Optional[float]
    paragraph_boundary_f1: Optional[float]
    paragraph_boundary_precision: Optional[float]
    paragraph_boundary_recall: Optional[float]
    paragraph_count_similarity: Optional[float]
    prose_block_boundary_f1: Optional[float]
    prose_block_boundary_precision: Optional[float]
    prose_block_boundary_recall: Optional[float]
    prose_block_count_similarity: Optional[float]
    # Text-content quality metrics
    bleu4: Optional[float]
    rouge1: Optional[float]
    rouge2: Optional[float]
    rougeL: Optional[float]
    cer: Optional[float]
    wer: Optional[float]
    f1_token: Optional[float]
    word_fragmentation_score: Optional[float]
    word_boundary_integrity_score: Optional[float]
    token_boundary_f1: Optional[float]
    text_quality_score: Optional[float]
    prediction_available: bool

    def to_json(self) -> Dict[str, Any]:
        return {
            "document_id": self.document_id,
            "scores": {
                "overall": self.overall,
                # Structural metrics
                "nid": self.nid,
                "nid_s": self.nid_s,
                "teds": self.teds,
                "teds_s": self.teds_s,
                "mhs": self.mhs,
                "mhs_s": self.mhs_s,
                "paragraph_boundary_f1": self.paragraph_boundary_f1,
                "paragraph_boundary_precision": self.paragraph_boundary_precision,
                "paragraph_boundary_recall": self.paragraph_boundary_recall,
                "paragraph_count_similarity": self.paragraph_count_similarity,
                "prose_block_boundary_f1": self.prose_block_boundary_f1,
                "prose_block_boundary_precision": self.prose_block_boundary_precision,
                "prose_block_boundary_recall": self.prose_block_boundary_recall,
                "prose_block_count_similarity": self.prose_block_count_similarity,
                # Text-content quality metrics
                "bleu4": self.bleu4,
                "rouge1": self.rouge1,
                "rouge2": self.rouge2,
                "rougeL": self.rougeL,
                "cer": self.cer,
                "wer": self.wer,
                "f1_token": self.f1_token,
                "word_fragmentation_score": self.word_fragmentation_score,
                "word_boundary_integrity_score": self.word_boundary_integrity_score,
                "token_boundary_f1": self.token_boundary_f1,
                "text_quality_score": self.text_quality_score,
            },
            "prediction_available": self.prediction_available,
        }


def _read_text(path: Path) -> str:
    """Read UTF-8 text from ``path`` returning an empty string on failure."""

    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        logging.warning("Missing file: %s", path)
        return ""
    except UnicodeDecodeError:
        logging.warning("Failed to decode file as UTF-8: %s", path)
        return ""


def _safe_mean(values: Iterable[float]) -> Optional[float]:
    values = list(values)
    return fmean(values) if values else None


def _load_summary_metadata(summary_dir: Path) -> Dict[str, Any]:
    """Read the first ``summary.json`` file in ``summary_dir`` if it exists."""

    for summary_path in sorted(summary_dir.glob("summary.json")):
        try:
            with summary_path.open(encoding="utf-8") as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError) as exc:
            logging.warning("Failed to read summary file %s: %s", summary_path, exc)


def _evaluate_single_document(
    doc_id: str,
    gt_path: Path,
    pred_path: Path,
) -> DocumentScores:
    gt_markdown = _read_text(gt_path)
    pred_markdown = _read_text(pred_path)
    prediction_available = pred_path.is_file()

    nid, nid_s = evaluate_reading_order(gt_markdown, pred_markdown)
    teds, teds_s = evaluate_table(gt_markdown, pred_markdown)
    mhs, mhs_s = evaluate_heading_level(gt_markdown, pred_markdown)
    paragraph_metrics = evaluate_paragraph_structure(gt_markdown, pred_markdown)
    text_metrics = evaluate_text_quality(gt_markdown, pred_markdown)

    # Overall composite: structural quality (NID, TEDS, MHS) + text content
    # quality (ROUGE-1, ROUGE-L, BLEU-4).  TEDS and MHS are only included
    # when the document contains tables / headings respectively.
    overall_values = [
        v for v in (nid, teds, mhs, text_metrics["text_quality_score"])
        if v is not None
    ]
    overall_average = _safe_mean(overall_values)

    return DocumentScores(
        overall=overall_average,
        document_id=doc_id,
        nid=nid,
        nid_s=nid_s,
        teds=teds,
        teds_s=teds_s,
        mhs=mhs,
        mhs_s=mhs_s,
        paragraph_boundary_f1=paragraph_metrics["boundary_f1"],
        paragraph_boundary_precision=paragraph_metrics["boundary_precision"],
        paragraph_boundary_recall=paragraph_metrics["boundary_recall"],
        paragraph_count_similarity=paragraph_metrics["count_similarity"],
        prose_block_boundary_f1=paragraph_metrics["prose_block_boundary_f1"],
        prose_block_boundary_precision=paragraph_metrics["prose_block_boundary_precision"],
        prose_block_boundary_recall=paragraph_metrics["prose_block_boundary_recall"],
        prose_block_count_similarity=paragraph_metrics["prose_block_count_similarity"],
        bleu4=text_metrics["bleu4"],
        rouge1=text_metrics["rouge1"],
        rouge2=text_metrics["rouge2"],
        rougeL=text_metrics["rougeL"],
        cer=text_metrics["cer"],
        wer=text_metrics["wer"],
        f1_token=text_metrics["f1_token"],
        word_fragmentation_score=text_metrics["word_fragmentation_score"],
        word_boundary_integrity_score=text_metrics["word_boundary_integrity_score"],
        token_boundary_f1=text_metrics["token_boundary_f1"],
        text_quality_score=text_metrics["text_quality_score"],
        prediction_available=prediction_available,
    )


def _aggregate_document_scores(documents: List[DocumentScores]) -> Dict[str, Any]:
    """Compute mean scores across documents and return a serialisable payload."""

    overall_values = [doc.overall for doc in documents if doc.overall is not None]
    nid_values = [doc.nid for doc in documents if doc.nid is not None]
    nid_s_values = [doc.nid_s for doc in documents if doc.nid_s is not None]
    teds_values = [doc.teds for doc in documents if doc.teds is not None]
    teds_s_values = [doc.teds_s for doc in documents if doc.teds_s is not None]
    mhs_values = [doc.mhs for doc in documents if doc.mhs is not None]
    mhs_s_values = [doc.mhs_s for doc in documents if doc.mhs_s is not None]
    paragraph_boundary_f1_values = [
        doc.paragraph_boundary_f1
        for doc in documents
        if doc.paragraph_boundary_f1 is not None
    ]
    paragraph_boundary_precision_values = [
        doc.paragraph_boundary_precision
        for doc in documents
        if doc.paragraph_boundary_precision is not None
    ]
    paragraph_boundary_recall_values = [
        doc.paragraph_boundary_recall
        for doc in documents
        if doc.paragraph_boundary_recall is not None
    ]
    paragraph_count_similarity_values = [
        doc.paragraph_count_similarity
        for doc in documents
        if doc.paragraph_count_similarity is not None
    ]
    prose_block_boundary_f1_values = [
        doc.prose_block_boundary_f1
        for doc in documents
        if doc.prose_block_boundary_f1 is not None
    ]
    prose_block_boundary_precision_values = [
        doc.prose_block_boundary_precision
        for doc in documents
        if doc.prose_block_boundary_precision is not None
    ]
    prose_block_boundary_recall_values = [
        doc.prose_block_boundary_recall
        for doc in documents
        if doc.prose_block_boundary_recall is not None
    ]
    prose_block_count_similarity_values = [
        doc.prose_block_count_similarity
        for doc in documents
        if doc.prose_block_count_similarity is not None
    ]
    # Text-content quality
    bleu4_values         = [doc.bleu4              for doc in documents if doc.bleu4              is not None]
    rouge1_values        = [doc.rouge1             for doc in documents if doc.rouge1             is not None]
    rouge2_values        = [doc.rouge2             for doc in documents if doc.rouge2             is not None]
    rougeL_values        = [doc.rougeL             for doc in documents if doc.rougeL             is not None]
    cer_values           = [doc.cer                for doc in documents if doc.cer                is not None]
    wer_values           = [doc.wer                for doc in documents if doc.wer                is not None]
    f1_token_values      = [doc.f1_token           for doc in documents if doc.f1_token           is not None]
    word_fragmentation_values = [
        doc.word_fragmentation_score
        for doc in documents
        if doc.word_fragmentation_score is not None
    ]
    word_boundary_integrity_values = [
        doc.word_boundary_integrity_score
        for doc in documents
        if doc.word_boundary_integrity_score is not None
    ]
    token_boundary_f1_values = [
        doc.token_boundary_f1
        for doc in documents
        if doc.token_boundary_f1 is not None
    ]
    text_quality_values  = [doc.text_quality_score for doc in documents if doc.text_quality_score is not None]

    overall_mean = _safe_mean(overall_values)
    nid_mean = _safe_mean(nid_values)
    nid_s_mean = _safe_mean(nid_s_values)
    teds_mean = _safe_mean(teds_values)
    teds_s_mean = _safe_mean(teds_s_values)
    mhs_mean = _safe_mean(mhs_values)
    mhs_s_mean = _safe_mean(mhs_s_values)
    paragraph_boundary_f1_mean = _safe_mean(paragraph_boundary_f1_values)
    paragraph_boundary_precision_mean = _safe_mean(paragraph_boundary_precision_values)
    paragraph_boundary_recall_mean = _safe_mean(paragraph_boundary_recall_values)
    paragraph_count_similarity_mean = _safe_mean(paragraph_count_similarity_values)
    prose_block_boundary_f1_mean = _safe_mean(prose_block_boundary_f1_values)
    prose_block_boundary_precision_mean = _safe_mean(prose_block_boundary_precision_values)
    prose_block_boundary_recall_mean = _safe_mean(prose_block_boundary_recall_values)
    prose_block_count_similarity_mean = _safe_mean(prose_block_count_similarity_values)

    missing_predictions = sum(1 for doc in documents if not doc.prediction_available)

    return {
        "score": {
            "overall_mean": overall_mean,
            # Structural metrics
            "nid_mean": nid_mean,
            "nid_s_mean": nid_s_mean,
            "teds_mean": teds_mean,
            "teds_s_mean": teds_s_mean,
            "mhs_mean": mhs_mean,
            "mhs_s_mean": mhs_s_mean,
            "paragraph_boundary_f1_mean": paragraph_boundary_f1_mean,
            "paragraph_boundary_precision_mean": paragraph_boundary_precision_mean,
            "paragraph_boundary_recall_mean": paragraph_boundary_recall_mean,
            "paragraph_count_similarity_mean": paragraph_count_similarity_mean,
            "prose_block_boundary_f1_mean": prose_block_boundary_f1_mean,
            "prose_block_boundary_precision_mean": prose_block_boundary_precision_mean,
            "prose_block_boundary_recall_mean": prose_block_boundary_recall_mean,
            "prose_block_count_similarity_mean": prose_block_count_similarity_mean,
            # Text-content quality metrics
            "bleu4_mean":              _safe_mean(bleu4_values),
            "rouge1_mean":             _safe_mean(rouge1_values),
            "rouge2_mean":             _safe_mean(rouge2_values),
            "rougeL_mean":             _safe_mean(rougeL_values),
            "cer_mean":                _safe_mean(cer_values),
            "wer_mean":                _safe_mean(wer_values),
            "f1_token_mean":           _safe_mean(f1_token_values),
            "word_fragmentation_score_mean": _safe_mean(word_fragmentation_values),
            "word_boundary_integrity_score_mean": _safe_mean(word_boundary_integrity_values),
            "token_boundary_f1_mean": _safe_mean(token_boundary_f1_values),
            "text_quality_score_mean": _safe_mean(text_quality_values),
        },
        "nid_count": len(nid_values),
        "teds_count": len(teds_values),
        "mhs_count": len(mhs_values),
        "paragraph_boundary_count": len(paragraph_boundary_f1_values),
        "prose_block_boundary_count": len(prose_block_boundary_f1_values),
        "text_quality_count": len(text_quality_values),
        "missing_predictions": missing_predictions,
    }


def _logging_scores(
    scores: DocumentScores,
    engine_name: str,
    doc_id: str,
) -> None:
    overall = scores.overall
    nid = scores.nid
    nid_s = scores.nid_s
    teds = scores.teds
    teds_s = scores.teds_s
    mhs = scores.mhs
    mhs_s = scores.mhs_s
    paragraph_boundary_f1 = scores.paragraph_boundary_f1
    paragraph_count_similarity = scores.paragraph_count_similarity
    prose_block_boundary_f1 = scores.prose_block_boundary_f1
    prose_block_count_similarity = scores.prose_block_count_similarity

    def _fmt(v: Optional[float]) -> str:
        return f"{v:.3f}" if v is not None else "none "

    logging.info(
        "engine=%s document=%s overall=%s nid=%s nid_s=%s teds=%s teds_s=%s "
        "mhs=%s mhs_s=%s pbf1=%s prose_bf1=%s "
        "bleu4=%s rouge1=%s rougeL=%s cer=%s wer=%s f1_tok=%s frag=%s wbis=%s tbf1=%s tqs=%s",
        engine_name,
        doc_id,
        _fmt(overall),
        _fmt(nid),
        _fmt(nid_s),
        _fmt(teds),
        _fmt(teds_s),
        _fmt(mhs),
        _fmt(mhs_s),
        _fmt(paragraph_boundary_f1),
        _fmt(prose_block_boundary_f1),
        _fmt(scores.bleu4),
        _fmt(scores.rouge1),
        _fmt(scores.rougeL),
        _fmt(scores.cer),
        _fmt(scores.wer),
        _fmt(scores.f1_token),
        _fmt(scores.word_fragmentation_score),
        _fmt(scores.word_boundary_integrity_score),
        _fmt(scores.token_boundary_f1),
        _fmt(scores.text_quality_score),
    )


def _evaluate_engine_version(
    gt_dir: Path,
    prediction_dir: Path,
    output_filename: str,
    target_doc_id: Optional[str] = None,
) -> Optional[Path]:
    """Run evaluation for a single ``engine/version`` directory."""

    markdown_dir = prediction_dir / "markdown"
    if not markdown_dir.is_dir():
        logging.info("Skipping %s (no markdown directory)", prediction_dir)
        return None

    gt_paths = sorted(gt_dir.glob("*.md"))
    if not gt_paths:
        logging.error("No ground truth markdown files found in %s", gt_dir)
        return None

    documents: List[DocumentScores] = []

    engine_name = prediction_dir.name
    logging.info(
        "Evaluating engine=%s with %d documents",
        engine_name,
        len(gt_paths),
    )

    for gt_path in gt_paths:
        doc_id = gt_path.stem
        if target_doc_id and doc_id != target_doc_id:
            continue

        pred_path = markdown_dir / f"{doc_id}.md"
        try:
            scores = _evaluate_single_document(doc_id, gt_path, pred_path)
            _logging_scores(scores, engine_name, doc_id)
        except Exception as exc:  # pragma: no cover - defensive guard
            logging.exception("Failed to evaluate %s: %s", doc_id, exc)
            continue
        documents.append(scores)

    if not documents:
        logging.warning("No documents evaluated for %s", prediction_dir)
        return None

    summary_metadata = _load_summary_metadata(prediction_dir)

    aggregated = _aggregate_document_scores(documents)
    payload = {
        "schema_version": CURRENT_EVALUATION_SCHEMA_VERSION,
        "summary": summary_metadata,
        "metrics": aggregated,
        "documents": [doc.to_json() for doc in documents],
    }

    output_path = prediction_dir / output_filename
    output_path.write_text(json.dumps(payload, indent=2, ensure_ascii=False))
    logging.info("Wrote evaluation to %s", output_path)

    csv_filename = Path(output_filename).with_suffix(".csv").name
    csv_path = prediction_dir / csv_filename
    csv_fieldnames = [
        "index",
        "document_id",
        "overall",
        "nid",
        "nid_s",
        "teds",
        "teds_s",
        "mhs",
        "mhs_s",
        "bleu4",
        "rouge1",
        "rouge2",
        "rougeL",
        "cer",
        "wer",
        "f1_token",
        "word_fragmentation_score",
        "word_boundary_integrity_score",
        "token_boundary_f1",
        "text_quality_score",
    ]
    with csv_path.open("w", encoding="utf-8", newline="") as csv_file:
        writer = csv.DictWriter(csv_file, fieldnames=csv_fieldnames)
        writer.writeheader()
        for index, doc in enumerate(documents):
            def _v(val: Optional[float]) -> str | float:
                return "" if val is None else val
            row = {
                "index": index + 1,
                "document_id": f"'{doc.document_id}",
                "overall":            _v(doc.overall),
                "nid":                _v(doc.nid),
                "nid_s":              _v(doc.nid_s),
                "teds":               _v(doc.teds),
                "teds_s":             _v(doc.teds_s),
                "mhs":                _v(doc.mhs),
                "mhs_s":              _v(doc.mhs_s),
                "bleu4":              _v(doc.bleu4),
                "rouge1":             _v(doc.rouge1),
                "rouge2":             _v(doc.rouge2),
                "rougeL":             _v(doc.rougeL),
                "cer":                _v(doc.cer),
                "wer":                _v(doc.wer),
                "f1_token":           _v(doc.f1_token),
                "word_fragmentation_score": _v(doc.word_fragmentation_score),
                "word_boundary_integrity_score": _v(doc.word_boundary_integrity_score),
                "token_boundary_f1": _v(doc.token_boundary_f1),
                "text_quality_score": _v(doc.text_quality_score),
            }
            writer.writerow(row)
    logging.info("Wrote evaluation CSV to %s", csv_path)
    return output_path


def run(
    ground_truth_dir_name: str,
    prediction_root_name: str,
    output_filename: str,
    target_engine: Optional[str] = None,
    target_doc_id: Optional[str] = None,
) -> List[Path]:
    """Evaluate engine/version pairs under ``prediction_root`` optionally filtered to a single document."""
    project_root = Path(__file__).parent.parent.resolve()

    ground_truth_dir = project_root / ground_truth_dir_name
    prediction_root = project_root / prediction_root_name

    if not ground_truth_dir.is_dir():
        raise FileNotFoundError(f"Ground truth directory not found: {ground_truth_dir}")

    if not prediction_root.is_dir():
        raise FileNotFoundError(f"Prediction directory not found: {prediction_root}")

    start_time = time.time()

    generated_files: List[Path] = []

    if target_engine:
        engine_dirs = [prediction_root / target_engine]
        if not engine_dirs[0].is_dir():
            logging.warning("Engine directory not found: %s", engine_dirs[0])
            engine_dirs = []
    else:
        engine_dirs = [p for p in sorted(prediction_root.iterdir()) if p.is_dir()]

    for engine_dir in engine_dirs:
        result_path = _evaluate_engine_version(
            ground_truth_dir, engine_dir, output_filename, target_doc_id
        )
        if result_path:
            generated_files.append(result_path)

    end_time = time.time()
    total_elapsed = end_time - start_time
    logging.info(
        "Completed evaluation of %d engine versions in %.2f seconds",
        len(generated_files),
        total_elapsed,
    )

    return generated_files


def _parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate Markdown predictions")
    parser.add_argument(
        "--ground-truth-dir",
        type=str,
        default=DEFAULT_GT_DIR,
        help="Directory containing ground-truth markdown files",
    )
    parser.add_argument(
        "--prediction-root",
        type=str,
        default=DEFAULT_PREDICTION_ROOT,
        help="Directory containing engine prediction outputs",
    )
    parser.add_argument(
        "--engine",
        type=str,
        default=None,
        help="Name of the engine to evaluate. If not specified, all engines are evaluated.",
    )
    parser.add_argument(
        "--doc-id",
        type=str,
        default=None,
        help="Evaluate only the specified document ID",
    )
    parser.add_argument(
        "--output-filename",
        type=str,
        default=DEFAULT_OUTPUT_FILENAME,
        help="Filename for generated evaluation JSON (placed in each version dir)",
    )
    parser.add_argument(
        "--log-level",
        type=str,
        choices=list(logging.getLevelNamesMapping().keys()),
        default="INFO",
        help="Python logging level (e.g. INFO, DEBUG)",
    )
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> None:
    args = _parse_args(argv)
    logging.basicConfig(level=getattr(logging, args.log_level.upper(), logging.INFO))
    generated = run(
        args.ground_truth_dir,
        args.prediction_root,
        args.output_filename,
        target_engine=args.engine,
        target_doc_id=args.doc_id,
    )
    for path in generated:
        print(path)


if __name__ == "__main__":  # pragma: no cover - CLI entry point
    main()
