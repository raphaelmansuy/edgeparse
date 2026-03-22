#!/usr/bin/env python3
"""Compare one prediction against benchmark ground truth with richer diagnostics."""

from __future__ import annotations

import argparse
from collections import Counter
import json
import re
import sys
import unicodedata
from dataclasses import dataclass
from difflib import SequenceMatcher
from pathlib import Path
from typing import Iterable

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "src"))

from evaluator_heading_level import evaluate_heading_level
from evaluator_paragraph import (
    evaluate_paragraph_structure,
    split_prose_blocks,
    split_text_paragraphs,
)
from evaluator_reading_order import evaluate_reading_order
from evaluator_table import evaluate_table


TABLE_SEPARATOR_RE = re.compile(r"^\s*\|[\s:]*-+[\s:]*\|", re.MULTILINE)
TABLE_BLOCK_RE = re.compile(r"(?:^\|.*\n)+", re.MULTILINE)
HTML_TABLE_RE = re.compile(r"<table>.*?</table>", re.IGNORECASE | re.DOTALL)
HTML_TAG_RE = re.compile(r"<[^>]+>")
ALNUM_RE = re.compile(r"\w+", re.UNICODE)


@dataclass
class MarkdownBlock:
    index: int
    kind: str
    text: str

    @property
    def normalized(self) -> str:
        return normalize_text(self.text)


def normalize_text(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def tokenize_words(text: str) -> list[str]:
    return [token.lower() for token in ALNUM_RE.findall(text)]


def extract_table_text(markdown: str) -> str:
    parts: list[str] = []
    parts.extend(TABLE_BLOCK_RE.findall(markdown))
    for html_table in HTML_TABLE_RE.findall(markdown):
        parts.append(HTML_TAG_RE.sub(" ", html_table))
    return "\n".join(parts)


def multiset_recall(gt_tokens: list[str], pred_tokens: list[str]) -> float:
    if not gt_tokens:
        return 1.0
    gt_counts = Counter(gt_tokens)
    pred_counts = Counter(pred_tokens)
    matched = sum(min(count, pred_counts[token]) for token, count in gt_counts.items())
    return matched / sum(gt_counts.values())


def lcs_token_recall(gt_tokens: list[str], pred_tokens: list[str]) -> float:
    if not gt_tokens:
        return 1.0
    if not pred_tokens:
        return 0.0

    prev = [0] * (len(pred_tokens) + 1)
    curr = [0] * (len(pred_tokens) + 1)
    for gt_token in gt_tokens:
        for j, pred_token in enumerate(pred_tokens, start=1):
            if gt_token == pred_token:
                curr[j] = prev[j - 1] + 1
            else:
                curr[j] = max(prev[j], curr[j - 1])
        prev, curr = curr, [0] * (len(pred_tokens) + 1)
    return prev[-1] / len(gt_tokens)


def is_non_latin_token(token: str) -> bool:
    for ch in token:
        if not ch.isalpha():
            continue
        try:
            name = unicodedata.name(ch)
        except ValueError:
            return True
        if "LATIN" not in name:
            return True
    return False


def missing_unique_tokens(gt_tokens: list[str], pred_tokens: list[str], *, predicate=None, limit: int = 20) -> list[str]:
    gt_counts = Counter(token for token in gt_tokens if predicate is None or predicate(token))
    pred_counts = Counter(token for token in pred_tokens if predicate is None or predicate(token))
    missing: list[str] = []
    for token in sorted(gt_counts):
        if pred_counts[token] < gt_counts[token]:
            missing.append(token)
        if len(missing) >= limit:
            break
    return missing


def split_markdown_blocks(markdown: str) -> list[MarkdownBlock]:
    lines = markdown.splitlines()
    blocks: list[MarkdownBlock] = []
    index = 0
    i = 0
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            i += 1
            continue

        if line.lstrip().startswith("|"):
            table_lines = [line]
            i += 1
            while i < len(lines) and lines[i].lstrip().startswith("|"):
                table_lines.append(lines[i])
                i += 1
            blocks.append(MarkdownBlock(index=index, kind="table", text="\n".join(table_lines)))
            index += 1
            continue

        kind = "heading" if line.lstrip().startswith("#") else "text"
        text_lines = [line]
        i += 1
        while i < len(lines) and lines[i].strip() and not lines[i].lstrip().startswith("|"):
            if lines[i].lstrip().startswith("#") and kind != "heading":
                break
            text_lines.append(lines[i])
            i += 1
        blocks.append(MarkdownBlock(index=index, kind=kind, text="\n".join(text_lines)))
        index += 1

    return blocks


def greedy_block_matches(
    gt_blocks: list[MarkdownBlock], pred_blocks: list[MarkdownBlock], min_score: float = 0.45
) -> list[tuple[MarkdownBlock, MarkdownBlock, float]]:
    remaining = {block.index for block in pred_blocks}
    matches: list[tuple[MarkdownBlock, MarkdownBlock, float]] = []

    for gt_block in gt_blocks:
        best_pred: MarkdownBlock | None = None
        best_score = 0.0
        for pred_block in pred_blocks:
            if pred_block.index not in remaining:
                continue
            score = SequenceMatcher(None, gt_block.normalized, pred_block.normalized).ratio()
            if pred_block.kind == gt_block.kind:
                score += 0.05
            if score > best_score:
                best_score = score
                best_pred = pred_block
        if best_pred is not None and best_score >= min_score:
            remaining.remove(best_pred.index)
            matches.append((gt_block, best_pred, min(best_score, 1.0)))

    return matches


def suspicious_table_reason(block: MarkdownBlock) -> str | None:
    if block.kind != "table":
        return None

    lines = [line.strip() for line in block.text.splitlines() if line.strip()]
    content_lines = [line for line in lines if not TABLE_SEPARATOR_RE.match(line)]
    if not content_lines:
        return "empty-table"

    rows = [split_pipe_row(line) for line in content_lines]
    max_cols = max(len(row) for row in rows)
    joined = " ".join(cell for row in rows for cell in row)
    normalized = normalize_text(joined)
    lower = normalized.lower()
    words = tokenize_words(joined)
    alpha_words = [word for word in words if any(ch.isalpha() for ch in word)]
    single_letter_words = [word for word in alpha_words if len(word) == 1]
    digit_count = sum(ch.isdigit() for ch in joined)
    percent_count = joined.count("%")

    if max_cols == 1 and ("question" in lower or "discussion" in lower) and ":" in normalized:
        return "boxed-prompt"
    if max_cols == 2 and (lower.startswith("figure ") or lower.startswith("diagram ")):
        return "caption-as-table"
    if max_cols == 1 and percent_count >= 2 and digit_count >= 4 and len(single_letter_words) * 2 >= max(len(alpha_words), 1):
        return "chart-label-cloud"
    if max_cols == 1 and len(words) >= 40 and any(mark in normalized for mark in [".", "?", "!"]):
        return "prose-sidebar"
    return None


def split_pipe_row(line: str) -> list[str]:
    cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
    return [cell for cell in cells if cell]


def text_fragmentation_score(text: str) -> float:
    alpha_words = [word for word in tokenize_words(text) if any(ch.isalpha() for ch in word)]
    if not alpha_words:
        return 0.0
    single_letter_words = sum(1 for word in alpha_words if len(word) == 1)
    return single_letter_words / len(alpha_words)


def windowed_order_score(gt_blocks: Iterable[MarkdownBlock], pred_blocks: Iterable[MarkdownBlock], window: int = 3) -> float:
    gt_tokens = [block.normalized for block in gt_blocks if block.kind != "table" and block.normalized]
    pred_tokens = [block.normalized for block in pred_blocks if block.kind != "table" and block.normalized]
    if not gt_tokens:
        return 1.0

    gt_windows = {" || ".join(gt_tokens[i:i + window]) for i in range(max(len(gt_tokens) - window + 1, 1))}
    pred_windows = {" || ".join(pred_tokens[i:i + window]) for i in range(max(len(pred_tokens) - window + 1, 1))}
    if not gt_windows:
        return 1.0
    return len(gt_windows & pred_windows) / len(gt_windows)


def starts_with_lowercase_word(text: str) -> bool:
    for ch in text.strip():
        if ch.isalpha():
            return ch.islower()
    return False


def looks_like_listish_block(text: str) -> bool:
    stripped = text.lstrip()
    return stripped.startswith(("-", "*", "•", "·")) or re.match(r"^\d+[.)]\s", stripped) is not None


def orphan_split_diagnostics(
    gt_blocks: list[MarkdownBlock], pred_blocks: list[MarkdownBlock]
) -> list[tuple[int, int, int, float, str]]:
    gt_text_blocks = [block for block in gt_blocks if block.kind == "text" and block.normalized]
    findings: list[tuple[int, int, int, float, str]] = []

    for idx in range(1, len(pred_blocks)):
        prev_block = pred_blocks[idx - 1]
        orphan_block = pred_blocks[idx]
        if prev_block.kind != "text" or orphan_block.kind != "text":
            continue
        if looks_like_listish_block(prev_block.text) or looks_like_listish_block(orphan_block.text):
            continue

        orphan_text = orphan_block.normalized
        if not orphan_text or len(orphan_text.split()) > 6:
            continue
        if not starts_with_lowercase_word(orphan_text):
            continue
        if prev_block.normalized.endswith((".", "!", "?", ":", ";")):
            continue

        combined = normalize_text(f"{prev_block.text} {orphan_block.text}")
        best_gt_index = -1
        best_combined = 0.0
        for gt_block in gt_text_blocks:
            gt_normalized = gt_block.normalized
            combined_score = SequenceMatcher(None, gt_block.normalized, combined).ratio()
            if combined_score > best_combined:
                best_combined = combined_score
                best_gt_index = gt_block.index
        combined_lower = combined.lower()
        gt_support = any(combined_lower in block.normalized.lower() for block in gt_text_blocks)

        if best_gt_index >= 0 and best_combined >= 0.88 and gt_support:
            findings.append((best_gt_index, prev_block.index, orphan_block.index, best_combined, preview(orphan_text)))

    return findings


def merged_paragraph_diagnostics(
    gt_markdown: str, pred_markdown: str
) -> list[tuple[int, int, int, float, str]]:
    gt_paragraphs = split_text_paragraphs(gt_markdown)
    pred_paragraphs = split_text_paragraphs(pred_markdown)
    findings: list[tuple[int, int, int, float, str]] = []

    for pred_idx, pred_paragraph in enumerate(pred_paragraphs):
        pred_text = normalize_text(pred_paragraph)
        if len(pred_text.split()) < 12 or looks_like_listish_block(pred_paragraph):
            continue

        best: tuple[int, int, float] | None = None
        for idx in range(len(gt_paragraphs) - 1):
            combined = normalize_text(f"{gt_paragraphs[idx]} {gt_paragraphs[idx + 1]}")
            score = SequenceMatcher(None, combined, pred_text).ratio()
            if best is None or score > best[2]:
                best = (idx, idx + 1, score)

        if best is None:
            continue
        left_idx, right_idx, score = best
        if score < 0.9:
            continue

        findings.append((left_idx, right_idx, pred_idx, score, preview(pred_paragraph)))

    return findings


def build_side_by_side_paragraphs(
    gt_markdown: str,
    pred_markdown: str,
    engine: str,
) -> list[str]:
    gt_paragraphs = split_text_paragraphs(gt_markdown)
    pred_paragraphs = split_text_paragraphs(pred_markdown)
    rows = [
        "| GT idx | Ground truth paragraph | Pred idx | " + engine + " paragraph |",
        "| --- | --- | --- | --- |",
    ]
    for idx in range(max(len(gt_paragraphs), len(pred_paragraphs))):
        gt_text = preview(gt_paragraphs[idx], 160) if idx < len(gt_paragraphs) else "<missing>"
        pred_text = preview(pred_paragraphs[idx], 160) if idx < len(pred_paragraphs) else "<missing>"
        gt_text = gt_text.replace("|", "\\|")
        pred_text = pred_text.replace("|", "\\|")
        rows.append(f"| {idx} | {gt_text} | {idx} | {pred_text} |")
    return rows


def build_side_by_side_prose_blocks(
    gt_markdown: str,
    pred_markdown: str,
    engine: str,
) -> list[str]:
    gt_blocks = split_prose_blocks(gt_markdown)
    pred_blocks = split_prose_blocks(pred_markdown)
    rows = [
        "| GT idx | Ground truth prose block | Pred idx | " + engine + " prose block |",
        "| --- | --- | --- | --- |",
    ]
    for idx in range(max(len(gt_blocks), len(pred_blocks))):
        gt_text = preview(gt_blocks[idx], 160) if idx < len(gt_blocks) else "<missing>"
        pred_text = preview(pred_blocks[idx], 160) if idx < len(pred_blocks) else "<missing>"
        gt_text = gt_text.replace("|", "\\|")
        pred_text = pred_text.replace("|", "\\|")
        rows.append(f"| {idx} | {gt_text} | {idx} | {pred_text} |")
    return rows


def build_report(doc_id: str, engine: str, gt_markdown: str, pred_markdown: str, reference_doc: dict) -> str:
    nid, nid_s = evaluate_reading_order(gt_markdown, pred_markdown)
    teds, teds_s = evaluate_table(gt_markdown, pred_markdown)
    mhs, mhs_s = evaluate_heading_level(gt_markdown, pred_markdown)
    paragraph_metrics = evaluate_paragraph_structure(gt_markdown, pred_markdown)

    gt_blocks = split_markdown_blocks(gt_markdown)
    pred_blocks = split_markdown_blocks(pred_markdown)
    gt_tokens = tokenize_words(gt_markdown)
    pred_tokens = tokenize_words(pred_markdown)
    gt_table_tokens = tokenize_words(extract_table_text(gt_markdown))
    pred_table_tokens = tokenize_words(extract_table_text(pred_markdown))
    block_matches = greedy_block_matches(gt_blocks, pred_blocks)
    orphan_splits = orphan_split_diagnostics(gt_blocks, pred_blocks)
    merged_paragraphs = merged_paragraph_diagnostics(gt_markdown, pred_markdown)
    matched_gt = {gt.index for gt, _, _ in block_matches}
    matched_pred = {pred.index for _, pred, _ in block_matches}

    block_precision = len(block_matches) / len(pred_blocks) if pred_blocks else 1.0
    block_recall = len(block_matches) / len(gt_blocks) if gt_blocks else 1.0
    block_f1 = (
        2 * block_precision * block_recall / (block_precision + block_recall)
        if (block_precision + block_recall)
        else 0.0
    )

    suspicious_tables = []
    for block in pred_blocks:
        reason = suspicious_table_reason(block)
        if reason:
            suspicious_tables.append((block.index, reason, preview(block.text)))

    report = []
    report.append(f"# {doc_id} vs ground truth")
    report.append("")
    report.append("## Existing benchmark metrics")
    report.append("")
    report.append(f"- Engine: `{engine}`")
    report.append(f"- NID: {fmt(nid)}")
    report.append(f"- NID-S: {fmt(nid_s)}")
    report.append(f"- TEDS: {fmt(teds)}")
    report.append(f"- TEDS-S: {fmt(teds_s)}")
    report.append(f"- MHS: {fmt(mhs)}")
    report.append(f"- MHS-S: {fmt(mhs_s)}")
    report.append("")
    report.append("## Proposed auxiliary metrics")
    report.append("")
    report.append(f"- Block alignment precision: {block_precision:.4f}")
    report.append(f"- Block alignment recall: {block_recall:.4f}")
    report.append(f"- Block alignment F1: {block_f1:.4f}")
    report.append(f"- GT block count: {len(gt_blocks)}")
    report.append(f"- Predicted block count: {len(pred_blocks)}")
    report.append(f"- Windowed reading-order score: {windowed_order_score(gt_blocks, pred_blocks):.4f}")
    gt_text_block_count = max(sum(block.kind == "text" for block in gt_blocks), 1)
    continuity_score = 1.0 - len(orphan_splits) / gt_text_block_count
    report.append(f"- Paragraph continuity score: {continuity_score:.4f}")
    report.append(f"- Predicted orphan continuation splits: {len(orphan_splits)}")
    report.append(f"- Paragraph boundary precision: {fmt(paragraph_metrics['boundary_precision'])}")
    report.append(f"- Paragraph boundary recall: {fmt(paragraph_metrics['boundary_recall'])}")
    report.append(f"- Paragraph boundary F1: {fmt(paragraph_metrics['boundary_f1'])}")
    report.append(f"- Paragraph count similarity: {fmt(paragraph_metrics['count_similarity'])}")
    report.append(f"- Paragraph count (GT vs Pred): {paragraph_metrics['gt_count']} vs {paragraph_metrics['pred_count']}")
    report.append(f"- Prose-block boundary precision: {fmt(paragraph_metrics['prose_block_boundary_precision'])}")
    report.append(f"- Prose-block boundary recall: {fmt(paragraph_metrics['prose_block_boundary_recall'])}")
    report.append(f"- Prose-block boundary F1: {fmt(paragraph_metrics['prose_block_boundary_f1'])}")
    report.append(f"- Prose-block count similarity: {fmt(paragraph_metrics['prose_block_count_similarity'])}")
    report.append(
        f"- Prose-block count (GT vs Pred): {paragraph_metrics['gt_prose_block_count']} vs {paragraph_metrics['pred_prose_block_count']}"
    )
    report.append(f"- Detected merged GT paragraph pairs: {len(merged_paragraphs)}")
    report.append(f"- Prediction fragmentation score: {text_fragmentation_score(pred_markdown):.4f}")
    report.append(f"- Ground-truth fragmentation score: {text_fragmentation_score(gt_markdown):.4f}")
    report.append(f"- Token coverage recall: {multiset_recall(gt_tokens, pred_tokens):.4f}")
    report.append(f"- Token LCS recall: {lcs_token_recall(gt_tokens, pred_tokens):.4f}")
    report.append(
        f"- Non-Latin token recall: {multiset_recall([t for t in gt_tokens if is_non_latin_token(t)], pred_tokens):.4f}"
    )
    report.append(
        f"- Numeric token recall: {multiset_recall([t for t in gt_tokens if any(ch.isdigit() for ch in t)], pred_tokens):.4f}"
    )
    report.append(f"- Table token recall: {multiset_recall(gt_table_tokens, pred_table_tokens):.4f}")
    report.append(f"- Predicted table blocks: {sum(block.kind == 'table' for block in pred_blocks)}")
    report.append(f"- Suspicious predicted table blocks: {len(suspicious_tables)}")
    if suspicious_tables:
        report.append("  These often capture prompt boxes, caption boxes, chart label clouds, or prose sidebars better than TD-F1 alone.")
    missing_non_latin = missing_unique_tokens(gt_tokens, pred_tokens, predicate=is_non_latin_token)
    missing_table_tokens = missing_unique_tokens(gt_table_tokens, pred_table_tokens)
    if missing_non_latin:
        report.append(f"- Missing non-Latin GT tokens: {', '.join(missing_non_latin)}")
    if missing_table_tokens:
        report.append(f"- Missing GT table tokens: {', '.join(missing_table_tokens)}")
    report.append("")
    report.append("## Ground-truth semantic elements")
    report.append("")
    for element in reference_doc.get("elements", []):
        category = element.get("category", "Unknown")
        page = element.get("page", "?")
        coords = element.get("coordinates", [])
        bbox = preview_bbox(coords)
        text = normalize_text(element.get("content", {}).get("text", ""))
        report.append(f"- Page {page} | {category} | {bbox} | {preview(text)}")
    report.append("")
    report.append("## Block alignment")
    report.append("")
    for gt_block, pred_block, score in block_matches:
        report.append(f"- GT[{gt_block.index}] `{gt_block.kind}` <-> Pred[{pred_block.index}] `{pred_block.kind}` score={score:.3f}")
        report.append(f"  GT: {preview(gt_block.text)}")
        report.append(f"  Pred: {preview(pred_block.text)}")
    unmatched_gt = [block for block in gt_blocks if block.index not in matched_gt]
    unmatched_pred = [block for block in pred_blocks if block.index not in matched_pred]
    if unmatched_gt:
        report.append("")
        report.append("## Unmatched ground-truth blocks")
        report.append("")
        for block in unmatched_gt:
            report.append(f"- GT[{block.index}] `{block.kind}`: {preview(block.text)}")
    if unmatched_pred:
        report.append("")
        report.append("## Unmatched predicted blocks")
        report.append("")
        for block in unmatched_pred:
            extra = suspicious_table_reason(block)
            suffix = f" [{extra}]" if extra else ""
            report.append(f"- Pred[{block.index}] `{block.kind}`{suffix}: {preview(block.text)}")
    if suspicious_tables:
        report.append("")
        report.append("## Suspicious predicted tables")
        report.append("")
        for block_index, reason, snippet in suspicious_tables:
            report.append(f"- Pred[{block_index}] {reason}: {snippet}")
    if orphan_splits:
        report.append("")
        report.append("## Paragraph continuity findings")
        report.append("")
        for gt_idx, prev_idx, orphan_idx, score, orphan_text in orphan_splits:
            report.append(
                f"- GT[{gt_idx}] is likely split across Pred[{prev_idx}] + Pred[{orphan_idx}] score={score:.3f}; orphan tail: {orphan_text}"
            )
    if merged_paragraphs:
        report.append("")
        report.append("## Paragraph boundary loss findings")
        report.append("")
        for left_idx, right_idx, pred_idx, score, snippet in merged_paragraphs:
            report.append(
                f"- Pred paragraph {pred_idx} likely merges GT paragraphs {left_idx} + {right_idx} score={score:.3f}; predicted text: {snippet}"
            )
    report.append("")
    report.append("## Paragraph Side By Side")
    report.append("")
    report.extend(build_side_by_side_paragraphs(gt_markdown, pred_markdown, engine))
    report.append("")
    report.append("## Prose Block Side By Side")
    report.append("")
    report.extend(build_side_by_side_prose_blocks(gt_markdown, pred_markdown, engine))
    report.append("")
    report.append("## Ground truth markdown")
    report.append("")
    report.append("```markdown")
    report.append(gt_markdown.rstrip())
    report.append("```")
    report.append("")
    report.append(f"## {engine} markdown")
    report.append("")
    report.append("```markdown")
    report.append(pred_markdown.rstrip())
    report.append("```")
    report.append("")
    return "\n".join(report)


def preview(text: str, limit: int = 220) -> str:
    text = normalize_text(text)
    if not text:
        return "<empty>"
    return text if len(text) <= limit else text[: limit - 1] + "..."


def preview_bbox(coordinates: list[dict]) -> str:
    if not coordinates:
        return "bbox=?"
    xs = [point["x"] for point in coordinates]
    ys = [point["y"] for point in coordinates]
    return f"bbox=({min(xs):.3f},{min(ys):.3f})-({max(xs):.3f},{max(ys):.3f})"


def fmt(value: float | None) -> str:
    return "N/A" if value is None else f"{value:.4f}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("doc_id", help="Document ID, for example 01030000000158")
    parser.add_argument("--engine", default="edgeparse", help="Prediction engine name under benchmark/prediction")
    parser.add_argument("--prediction-root", default=str(ROOT / "prediction"))
    parser.add_argument("--ground-truth-dir", default=str(ROOT / "ground-truth"))
    parser.add_argument("--output", default=None, help="Optional report output path")
    args = parser.parse_args()

    gt_dir = Path(args.ground_truth_dir)
    pred_root = Path(args.prediction_root)
    gt_markdown_path = gt_dir / "markdown" / f"{args.doc_id}.md"
    pred_markdown_path = pred_root / args.engine / "markdown" / f"{args.doc_id}.md"
    reference_path = gt_dir / "reference.json"

    gt_markdown = gt_markdown_path.read_text(encoding="utf-8")
    pred_markdown = pred_markdown_path.read_text(encoding="utf-8")
    reference = json.loads(reference_path.read_text(encoding="utf-8"))
    reference_doc = reference[f"{args.doc_id}.pdf"]

    report = build_report(args.doc_id, args.engine, gt_markdown, pred_markdown, reference_doc)

    if args.output:
        output_path = Path(args.output)
        output_path.write_text(report, encoding="utf-8")
    else:
        print(report)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
