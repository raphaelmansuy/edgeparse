"""Evaluation payload schema helpers."""

from __future__ import annotations

from typing import Any, Dict, List


CURRENT_EVALUATION_SCHEMA_VERSION = 6

REQUIRED_SCORE_KEYS = (
    "overall_mean",
    "nid_mean",
    "teds_mean",
    "table_cell_occupancy_f1_mean",
    "mhs_mean",
    "paragraph_boundary_f1_mean",
    "prose_block_boundary_f1_mean",
    "bleu4_mean",
    "rouge1_mean",
    "rouge2_mean",
    "rougeL_mean",
    "cer_mean",
    "wer_mean",
    "f1_token_mean",
    "word_fragmentation_score_mean",
    "word_boundary_integrity_score_mean",
    "token_boundary_f1_mean",
    "boundary_contamination_score_mean",
    "text_quality_score_mean",
)

REQUIRED_DOCUMENT_SCORE_KEYS = (
    "overall",
    "nid",
    "nid_s",
    "teds",
    "teds_s",
    "table_cell_occupancy_f1",
    "mhs",
    "mhs_s",
    "paragraph_boundary_f1",
    "prose_block_boundary_f1",
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
    "boundary_contamination_score",
    "text_quality_score",
)


def missing_evaluation_requirements(payload: Dict[str, Any]) -> List[str]:
    """Return unmet schema requirements for ``payload``."""

    missing: List[str] = []

    version = payload.get("schema_version")
    if version is None or version < CURRENT_EVALUATION_SCHEMA_VERSION:
        missing.append(f"schema_version>={CURRENT_EVALUATION_SCHEMA_VERSION}")

    scores = payload.get("metrics", {}).get("score", {})
    for key in REQUIRED_SCORE_KEYS:
        if key not in scores:
            missing.append(f"metrics.score.{key}")

    documents = payload.get("documents")
    if not isinstance(documents, list) or not documents:
        missing.append("documents[]")
        return missing

    doc_scores = documents[0].get("scores", {}) if isinstance(documents[0], dict) else {}
    for key in REQUIRED_DOCUMENT_SCORE_KEYS:
        if key not in doc_scores:
            missing.append(f"documents[].scores.{key}")

    return missing


def is_current_evaluation_payload(payload: Dict[str, Any]) -> bool:
    return not missing_evaluation_requirements(payload)
