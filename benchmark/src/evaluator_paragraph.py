"""Paragraph-aware structural evaluation."""

from __future__ import annotations

import re
from typing import Dict, List, Optional

WORD_RE = re.compile(r"\w+", re.UNICODE)
LIST_RE = re.compile(r"^(?:[-*+•]|\d+[.)])\s")
CAPTION_PREFIX_RE = re.compile(
    r"^(?:table|figure|fig\.|chart|graph|image|illustration|diagram|plate|map|exhibit)\s+\w+",
    re.IGNORECASE,
)
FOOTER_RE = re.compile(
    r"^(?:\d+\s*\|\s*[A-Z][A-Za-z0-9 .:'&()/-]+|[A-Z][A-Z0-9 .:'&()/-]+\s+\d+|\d+\s+[A-Z][A-Za-z0-9 .:'&()/-]+)$"
)


def _normalize(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def split_text_paragraphs(markdown: str) -> List[str]:
    return _split_structural_blocks(markdown, include_list_blocks=False)


def split_prose_blocks(markdown: str) -> List[str]:
    return _split_structural_blocks(markdown, include_list_blocks=True)


def _split_structural_blocks(markdown: str, *, include_list_blocks: bool) -> List[str]:
    paragraphs: List[str] = []
    current: List[str] = []

    def flush() -> None:
        if not current:
            return
        block = "\n".join(current).strip()
        current.clear()
        if not block:
            return
        lines = [line.strip() for line in block.splitlines() if line.strip()]
        first = next(iter(lines), "")
        if not first:
            return
        if first.startswith("#") or first.startswith("|") or first.startswith("<table"):
            return
        if LIST_RE.match(first):
            unbulleted = LIST_RE.sub("", first, count=1).strip()
            if CAPTION_PREFIX_RE.match(unbulleted):
                first = unbulleted
                block = "\n".join([unbulleted, *lines[1:]]).strip()
            elif not include_list_blocks:
                return
        normalized = _normalize(block)
        if not normalized:
            return
        if _looks_like_margin_header_fragment(normalized):
            return
        if _looks_like_running_footer(normalized):
            return
        if _looks_like_toc_block(lines, normalized):
            return
        if _looks_like_chart_label_cloud(lines, normalized):
            return
        if _looks_like_sparse_figure_label_cloud(lines, normalized):
            return
        if _looks_like_percent_bar_chart_cloud(lines, normalized):
            return
        if _looks_like_concept_map_cloud(lines, normalized):
            return
        paragraphs.append(normalized)

    for line in markdown.splitlines():
        if not line.strip():
            flush()
            continue
        current.append(line)
    flush()

    return paragraphs


def _looks_like_margin_header_fragment(text: str) -> bool:
    words = text.split()
    if not words or len(words) > 4 or len(text) > 24:
        return False

    alpha_chars = [ch for ch in text if ch.isalpha()]
    digit_count = sum(ch.isdigit() for ch in text)
    all_caps = bool(alpha_chars) and all(ch.isupper() for ch in alpha_chars)

    return all_caps or digit_count > 0


def _looks_like_running_footer(text: str) -> bool:
    if re.match(r"^\d+\s*\|\s*[A-Za-z]", text):
        return True
    if re.match(r"^[A-Za-z].*\|\s*\d+$", text):
        return True

    if not FOOTER_RE.match(text):
        return False

    alpha_chars = [ch for ch in text if ch.isalpha()]
    if not alpha_chars:
        return False

    upper_ratio = sum(ch.isupper() for ch in alpha_chars) / len(alpha_chars)
    return upper_ratio >= 0.55


def _looks_like_toc_block(lines: List[str], text: str) -> bool:
    if len(lines) < 3:
        return False

    lower = text.lower()
    if "contents" == lower.strip() or "table of contents" == lower.strip():
        return True

    page_like_lines = 0
    dotted_lines = 0
    for line in lines:
        stripped = line.strip()
        if "." * 3 in stripped:
            dotted_lines += 1
        tail = stripped.rsplit(maxsplit=1)[-1] if stripped.split() else ""
        if re.fullmatch(r"(?:\d+|[ivxlcdmIVXLCDM]+)", tail):
            page_like_lines += 1

    return (
        page_like_lines * 10 >= len(lines) * 7
        and dotted_lines * 10 >= len(lines) * 5
    )


def _looks_like_chart_label_cloud(lines: List[str], text: str) -> bool:
    if len(lines) < 4:
        return False

    tokens = text.split()
    if len(tokens) < 6:
        return False
    if any(mark in text for mark in [".", "?", "!", ":", ";"]):
        return False

    short_line_count = sum(1 for line in lines if len(line.split()) <= 4)
    numeric_tokens = sum(
        token.replace(",", "").replace(".", "").replace("%", "").isdigit()
        for token in tokens
    )
    percentish_tokens = sum("%" in token for token in tokens)
    avg_token_len = sum(len(token) for token in tokens) / len(tokens)

    return (
        short_line_count * 10 >= len(lines) * 7
        and (numeric_tokens + percentish_tokens) * 10 >= len(tokens) * 2
        and avg_token_len <= 8.5
    )


def _looks_like_sparse_figure_label_cloud(lines: List[str], text: str) -> bool:
    if len(lines) < 8:
        return False

    tokens = text.split()
    if len(tokens) < 12:
        return False

    short_line_count = sum(1 for line in lines if len(line.split()) <= 4)
    avg_words_per_line = sum(len(line.split()) for line in lines) / len(lines)
    mixed_case_tokens = sum(
        any(ch.islower() for ch in token) and any(ch.isupper() for ch in token)
        for token in tokens
    )
    stopword_count = sum(
        token.lower().strip(".,;:!?()[]{}\"'") in {
            "the", "and", "of", "to", "in", "for", "with", "on", "a", "an", "is"
        }
        for token in tokens
    )
    punctuation_chars = sum(ch in "/&-()" for ch in text)
    sentence_punct = sum(ch in ".?!" for ch in text)

    return (
        short_line_count * 10 >= len(lines) * 7
        and avg_words_per_line <= 3.8
        and mixed_case_tokens * 10 >= len(tokens) * 2
        and stopword_count * 8 <= len(tokens)
        and punctuation_chars >= 2
        and sentence_punct <= 1
    )


def _looks_like_percent_bar_chart_cloud(lines: List[str], text: str) -> bool:
    if len(lines) < 8:
        return False

    tokens = text.split()
    if len(tokens) < 16:
        return False

    avg_words_per_line = sum(len(line.split()) for line in lines) / len(lines)
    numeric_tokens = sum(
        token.replace(",", "").replace(".", "").replace("%", "").isdigit()
        for token in tokens
    )
    percentish_tokens = sum("%" in token for token in tokens)
    sentence_punct = sum(ch in ".?!" for ch in text)

    return (
        avg_words_per_line <= 6.2
        and percentish_tokens >= 3
        and numeric_tokens >= 6
        and sentence_punct <= 1
    )


def _looks_like_concept_map_cloud(lines: List[str], text: str) -> bool:
    if len(lines) < 16:
        return False

    tokens = text.split()
    if len(tokens) < 20:
        return False

    avg_words_per_line = sum(len(line.split()) for line in lines) / len(lines)
    mixed_case_tokens = sum(
        any(ch.islower() for ch in token) and any(ch.isupper() for ch in token)
        for token in tokens
    )
    punctuation_chars = sum(ch in "/&-()" for ch in text)
    stopword_count = sum(
        token.lower().strip(".,;:!?()[]{}\"'") in {
            "the", "and", "of", "to", "in", "for", "with", "on", "a", "an", "is"
        }
        for token in tokens
    )

    return (
        avg_words_per_line <= 3.5
        and mixed_case_tokens >= 4
        and punctuation_chars >= 3
        and stopword_count * 6 <= len(tokens)
    )


def _tokenize(text: str) -> List[str]:
    return [token.lower() for token in WORD_RE.findall(text)]


def _boundary_positions(paragraphs: List[str]) -> tuple[List[str], List[int]]:
    tokens: List[str] = []
    boundaries: List[int] = []
    for idx, paragraph in enumerate(paragraphs):
        para_tokens = _tokenize(paragraph)
        if not para_tokens:
            continue
        tokens.extend(para_tokens)
        if idx < len(paragraphs) - 1:
            boundaries.append(len(tokens))
    return tokens, boundaries


def _match_boundaries(gt_boundaries: List[int], pred_boundaries: List[int], tolerance: int = 12) -> int:
    matched = 0
    used = [False] * len(pred_boundaries)
    for gt_boundary in gt_boundaries:
        best_idx = None
        best_delta = None
        for idx, pred_boundary in enumerate(pred_boundaries):
            if used[idx]:
                continue
            delta = abs(pred_boundary - gt_boundary)
            if delta > tolerance:
                continue
            if best_delta is None or delta < best_delta:
                best_delta = delta
                best_idx = idx
        if best_idx is not None:
            used[best_idx] = True
            matched += 1
    return matched


def evaluate_paragraph_structure(gt: str, pred: str) -> Dict[str, Optional[float] | int]:
    gt_paragraphs = split_text_paragraphs(gt)
    pred_paragraphs = split_text_paragraphs(pred)
    prose_metrics = _evaluate_structural_lists(split_prose_blocks(gt), split_prose_blocks(pred))
    paragraph_metrics = _evaluate_structural_lists(gt_paragraphs, pred_paragraphs)

    return {
        **paragraph_metrics,
        "prose_block_boundary_precision": prose_metrics["boundary_precision"],
        "prose_block_boundary_recall": prose_metrics["boundary_recall"],
        "prose_block_boundary_f1": prose_metrics["boundary_f1"],
        "prose_block_count_similarity": prose_metrics["count_similarity"],
        "gt_prose_block_count": prose_metrics["gt_count"],
        "pred_prose_block_count": prose_metrics["pred_count"],
        "matched_prose_boundaries": prose_metrics["matched_boundaries"],
    }


def _evaluate_structural_lists(
    gt_paragraphs: List[str], pred_paragraphs: List[str]
) -> Dict[str, Optional[float] | int]:
    if not gt_paragraphs:
        return {
            "count_similarity": None,
            "boundary_precision": None,
            "boundary_recall": None,
            "boundary_f1": None,
            "gt_count": 0,
            "pred_count": len(pred_paragraphs),
            "matched_boundaries": 0,
        }

    gt_tokens, gt_boundaries = _boundary_positions(gt_paragraphs)
    pred_tokens, pred_boundaries = _boundary_positions(pred_paragraphs)

    matched_boundaries = _match_boundaries(gt_boundaries, pred_boundaries)
    precision = (
        matched_boundaries / len(pred_boundaries)
        if pred_boundaries
        else 1.0 if not gt_boundaries else 0.0
    )
    recall = (
        matched_boundaries / len(gt_boundaries)
        if gt_boundaries
        else 1.0
    )
    f1 = (
        2 * precision * recall / (precision + recall)
        if (precision + recall)
        else 0.0
    )
    count_similarity = 1.0 - (
        abs(len(gt_paragraphs) - len(pred_paragraphs))
        / max(len(gt_paragraphs), len(pred_paragraphs), 1)
    )

    if not gt_tokens and not pred_tokens:
        precision = recall = f1 = 1.0

    return {
        "count_similarity": count_similarity,
        "boundary_precision": precision,
        "boundary_recall": recall,
        "boundary_f1": f1,
        "gt_count": len(gt_paragraphs),
        "pred_count": len(pred_paragraphs),
        "matched_boundaries": matched_boundaries,
    }
