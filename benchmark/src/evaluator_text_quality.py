"""Text-content quality metrics for PDF-to-Markdown evaluation.

Computes BLEU-4, ROUGE-1/2/L, CER, WER, and F1-token from plain-text
representations of the ground-truth and predicted Markdown.

All metrics operate on normalised plain text (Markdown syntax stripped,
whitespace collapsed, lowercased) so that cosmetic formatting differences
do not inflate or deflate content-accuracy scores.

Metric summary
--------------
bleu4        BLEU-4 with +1 smoothing     [0–1]  higher is better
rouge1       ROUGE-1 F1                   [0–1]  higher is better
rouge2       ROUGE-2 F1                   [0–1]  higher is better
rougeL       ROUGE-L F1 (LCS-based)       [0–1]  higher is better
cer          Character Error Rate         [0–∞]  lower is better
wer          Word Error Rate              [0–∞]  lower is better
f1_token     Bag-of-words F1              [0–1]  higher is better
word_fragmentation_score  OCR split-word fidelity  [0–1]  higher is better
word_boundary_integrity_score  Preserves whole-word boundaries  [0–1]  higher is better
token_boundary_f1  Symmetric word-boundary fidelity  [0–1]  higher is better
text_quality_score  mean(rouge1, rougeL, bleu4, word_fragmentation_score, word_boundary_integrity_score, token_boundary_f1)  [0–1]  higher is better
"""

from __future__ import annotations

import math
import re
from collections import Counter
from difflib import SequenceMatcher
from typing import Dict, List, Optional, Tuple

from rapidfuzz.distance import Levenshtein


# ─── Text normalisation ────────────────────────────────────────────────────────

_CODE_BLOCK_RE   = re.compile(r"```[\s\S]*?```")
_INLINE_CODE_RE  = re.compile(r"`[^`]*`")
_HTML_TAG_RE     = re.compile(r"<[^>]+>")
_HEADING_RE      = re.compile(r"^#{1,6}\s+", re.MULTILINE)
_BOLD_ITALIC_RE  = re.compile(r"\*{1,3}([\s\S]*?)\*{1,3}")
_UNDERSCORE_RE   = re.compile(r"_{1,3}([\s\S]*?)_{1,3}")
_LINK_RE         = re.compile(r"!\[([^\]]*)\]\([^)]*\)")  # images first
_IMAGE_RE        = re.compile(r"\[([^\]]*)\]\([^)]*\)")   # then links
_MATH_BLOCK_RE   = re.compile(r"\$\$[\s\S]*?\$\$")
_MATH_INLINE_RE  = re.compile(r"\$[^$\n]+\$")
_TABLE_PIPE_RE   = re.compile(r"\|")
_TABLE_SEP_RE    = re.compile(r"^[\s|:\-]+$", re.MULTILINE)
_WHITESPACE_RE   = re.compile(r"\s+")
_WORD_RE         = re.compile(r"\w+", re.UNICODE)


def strip_markdown(text: str) -> str:
    """Remove Markdown / HTML formatting; return lowercased plain text."""
    # Fenced code blocks
    text = _CODE_BLOCK_RE.sub(" ", text)
    # Inline code
    text = _INLINE_CODE_RE.sub(" ", text)
    # HTML tags
    text = _HTML_TAG_RE.sub(" ", text)
    # Display math before inline math
    text = _MATH_BLOCK_RE.sub(" ", text)
    text = _MATH_INLINE_RE.sub(" ", text)
    # Headings — strip the `#` marker, keep the heading text
    text = _HEADING_RE.sub("", text)
    # Bold / italic — keep inner text
    while _BOLD_ITALIC_RE.search(text):
        text = _BOLD_ITALIC_RE.sub(r"\1", text)
    while _UNDERSCORE_RE.search(text):
        text = _UNDERSCORE_RE.sub(r"\1", text)
    # Images → alt text; links → link text
    text = _LINK_RE.sub(r"\1", text)
    text = _IMAGE_RE.sub(r"\1", text)
    # Table separators and pipes
    text = _TABLE_SEP_RE.sub(" ", text)
    text = _TABLE_PIPE_RE.sub(" ", text)
    # Collapse whitespace and lowercase
    text = _WHITESPACE_RE.sub(" ", text).strip().lower()
    return text


def _tokenize(text: str) -> List[str]:
    """Return list of word tokens (Unicode-aware)."""
    return _WORD_RE.findall(text)


# ─── BLEU-4 ───────────────────────────────────────────────────────────────────

def _count_ngrams(tokens: List[str], n: int) -> Counter:
    return Counter(tuple(tokens[i : i + n]) for i in range(max(len(tokens) - n + 1, 0)))


def _bleu4(ref_tokens: List[str], hyp_tokens: List[str]) -> float:
    """Corpus-level BLEU-4 with +1 (Chen-Cherry) smoothing.

    Returns 0.0 on empty inputs; never raises.
    """
    if not ref_tokens or not hyp_tokens:
        return 0.0

    # Brevity penalty
    r, c = len(ref_tokens), len(hyp_tokens)
    bp = 1.0 if c >= r else math.exp(1.0 - r / c)

    log_avg = 0.0
    for n in range(1, 5):
        ref_ng = _count_ngrams(ref_tokens, n)
        hyp_ng = _count_ngrams(hyp_tokens, n)

        match = sum(min(cnt, ref_ng.get(ng, 0)) for ng, cnt in hyp_ng.items())
        total = max(len(hyp_tokens) - n + 1, 0)

        # +1 smoothing prevents log(0)
        log_avg += math.log((match + 1) / (total + 1)) / 4

    return float(bp * math.exp(log_avg))


# ─── ROUGE-1 / ROUGE-2 ────────────────────────────────────────────────────────

def _rouge_n_f1(ref_tokens: List[str], hyp_tokens: List[str], n: int) -> float:
    """ROUGE-N F1 (uses unigrams for n=1, bigrams for n=2, etc.)."""
    ref_ng = _count_ngrams(ref_tokens, n)
    hyp_ng = _count_ngrams(hyp_tokens, n)

    if not ref_ng:
        return 0.0

    match = sum(min(cnt, ref_ng.get(ng, 0)) for ng, cnt in hyp_ng.items())

    ref_total = sum(ref_ng.values())
    hyp_total = sum(hyp_ng.values())

    if hyp_total == 0 or ref_total == 0:
        return 0.0

    precision = match / hyp_total
    recall    = match / ref_total

    if precision + recall == 0.0:
        return 0.0
    return 2.0 * precision * recall / (precision + recall)


# ─── ROUGE-L (LCS-based) ─────────────────────────────────────────────────────

def _lcs_len(a: List[str], b: List[str]) -> int:
    """Length of the Longest Common Subsequence of two token lists."""
    m, n = len(a), len(b)
    # Use two-row DP to keep memory linear
    prev: List[int] = [0] * (n + 1)
    for i in range(m):
        curr: List[int] = [0] * (n + 1)
        for j in range(n):
            curr[j + 1] = prev[j] + 1 if a[i] == b[j] else max(prev[j + 1], curr[j])
        prev = curr
    return prev[n]


def _rouge_l_f1(ref_tokens: List[str], hyp_tokens: List[str]) -> float:
    """ROUGE-L F1 based on Longest Common Subsequence."""
    if not ref_tokens or not hyp_tokens:
        return 0.0

    lcs = _lcs_len(ref_tokens, hyp_tokens)
    precision = lcs / len(hyp_tokens)
    recall    = lcs / len(ref_tokens)

    if precision + recall == 0.0:
        return 0.0
    return 2.0 * precision * recall / (precision + recall)


# ─── CER / WER ────────────────────────────────────────────────────────────────

def _cer(gt_plain: str, pred_plain: str) -> Optional[float]:
    """Character Error Rate = Levenshtein(chars) / len(reference).

    Capped at 2.0 to keep outliers from dominating averages.
    Returns None when the reference is empty.
    """
    if not gt_plain:
        return None
    dist = Levenshtein.distance(gt_plain, pred_plain)
    return min(dist / len(gt_plain), 2.0)


def _wer(gt_plain: str, pred_plain: str) -> Optional[float]:
    """Word Error Rate = Levenshtein(words) / len(reference_words).

    Capped at 2.0. Returns None when the reference has no words.
    """
    ref_words = gt_plain.split()
    hyp_words = pred_plain.split()
    if not ref_words:
        return None
    dist = Levenshtein.distance(ref_words, hyp_words)
    return min(dist / len(ref_words), 2.0)


# ─── F1-token (bag-of-words) ──────────────────────────────────────────────────

def _f1_token(ref_tokens: List[str], hyp_tokens: List[str]) -> float:
    """Bag-of-words token F1 (unordered unigram precision × recall harmonic mean).

    Equivalent to ROUGE-1 but computed from Counter directly.
    Handles multisets correctly (shared tokens counted up to their min frequency).
    """
    if not ref_tokens and not hyp_tokens:
        return 1.0
    if not ref_tokens or not hyp_tokens:
        return 0.0

    ref_c = Counter(ref_tokens)
    hyp_c = Counter(hyp_tokens)
    common = sum((ref_c & hyp_c).values())

    precision = common / len(hyp_tokens)
    recall    = common / len(ref_tokens)

    if precision + recall == 0.0:
        return 0.0
    return 2.0 * precision * recall / (precision + recall)


def _is_fragment_token(token: str) -> bool:
    return token.isalpha() and 1 <= len(token) <= 4


def _word_fragmentation_score(ref_tokens: List[str], hyp_tokens: List[str]) -> Optional[float]:
    """Score split-word corruption in the prediction.

    Detects adjacent short alphabetic hypothesis tokens whose concatenation
    matches a longer alphabetic reference token. A lower score means more OCR-
    style word shattering such as ``ow ne r ship`` for ``ownership``.
    """
    ref_long_words = Counter(
        token for token in ref_tokens if token.isalpha() and len(token) >= 6
    )
    total_candidates = sum(ref_long_words.values())
    if total_candidates == 0:
        return None

    fragmented_matches = 0
    fragmented_shards = 0
    i = 0
    while i < len(hyp_tokens):
        if not _is_fragment_token(hyp_tokens[i]):
            i += 1
            continue

        joined = hyp_tokens[i]
        matched = False
        for j in range(i + 1, min(i + 5, len(hyp_tokens))):
            if not _is_fragment_token(hyp_tokens[j]):
                break
            joined += hyp_tokens[j]
            if len(joined) >= 6 and ref_long_words.get(joined, 0) > 0:
                ref_long_words[joined] -= 1
                fragmented_matches += 1
                fragmented_shards += (j - i + 1)
                i = j + 1
                matched = True
                break

        if not matched:
            i += 1

    fragmentation_rate = fragmented_matches / total_candidates

    ref_alpha_count = sum(1 for token in ref_tokens if token.isalpha())
    hyp_alpha_count = sum(1 for token in hyp_tokens if token.isalpha())
    token_inflation_rate = 0.0
    if ref_alpha_count > 0 and hyp_alpha_count > ref_alpha_count:
        token_inflation_rate = (hyp_alpha_count - ref_alpha_count) / ref_alpha_count

    # Split words surface in two coupled ways:
    # 1. adjacent OCR shards can be rejoined into a reference word
    # 2. the prediction carries too many alphabetic tokens overall
    penalty = max(fragmentation_rate, min(token_inflation_rate, 1.0))
    return max(0.0, 1.0 - penalty)


def _word_boundary_integrity_score(
    ref_tokens: List[str],
    hyp_tokens: List[str],
) -> Optional[float]:
    """Score whether long reference words survive as intact units.

    This complements ``_word_fragmentation_score`` by penalizing the number of
    extra internal boundaries inserted into long alphabetic reference tokens.
    For example, ``ownership`` is correct, while ``ow ne r ship`` incurs three
    spurious internal boundaries.
    """
    ref_long_words = Counter(
        token for token in ref_tokens if token.isalpha() and len(token) >= 6
    )
    total_candidates = sum(ref_long_words.values())
    if total_candidates == 0:
        return None

    hyp_long_words = Counter(
        token for token in hyp_tokens if token.isalpha() and len(token) >= 6
    )
    intact_matches = 0
    for token, ref_count in list(ref_long_words.items()):
        if ref_count <= 0:
            continue
        intact = min(ref_count, hyp_long_words.get(token, 0))
        if intact > 0:
            intact_matches += intact
            ref_long_words[token] -= intact

    fragmented_credit = 0.0
    i = 0
    while i < len(hyp_tokens):
        if not _is_fragment_token(hyp_tokens[i]):
            i += 1
            continue

        joined = hyp_tokens[i]
        matched = False
        for j in range(i + 1, min(i + 6, len(hyp_tokens))):
            if not _is_fragment_token(hyp_tokens[j]):
                break
            joined += hyp_tokens[j]
            if len(joined) >= 6 and ref_long_words.get(joined, 0) > 0:
                ref_long_words[joined] -= 1
                fragmented_credit += 1.0 / (j - i + 1)
                i = j + 1
                matched = True
                break

        if not matched:
            i += 1

    return min((intact_matches + fragmented_credit) / total_candidates, 1.0)


def _token_boundary_positions(tokens: List[str]) -> List[int]:
    positions: List[int] = []
    cursor = 0
    for token in tokens[:-1]:
        cursor += len(token)
        positions.append(cursor)
    return positions


def _token_boundary_f1(ref_tokens: List[str], hyp_tokens: List[str]) -> Optional[float]:
    """Score whether word boundaries survive after whitespace is removed.

    Unlike the long-word fragmentation metrics, this is symmetric: it penalizes
    both inserted spaces inside a word and missing spaces between adjacent
    reference tokens.
    """
    if not ref_tokens:
        return None

    ref_compact = "".join(ref_tokens)
    hyp_compact = "".join(hyp_tokens)
    if not ref_compact:
        return None

    ref_boundaries = set(_token_boundary_positions(ref_tokens))
    hyp_boundaries = set(_token_boundary_positions(hyp_tokens))

    if not ref_boundaries and not hyp_boundaries:
        return 1.0
    if not ref_boundaries:
        return 0.0 if hyp_boundaries else 1.0
    if not hyp_boundaries:
        return 0.0

    matcher = SequenceMatcher(a=ref_compact, b=hyp_compact, autojunk=False)
    projected_hyp_boundaries = set()
    for ref_start, hyp_start, size in matcher.get_matching_blocks():
        if size <= 1:
            continue
        for pos in hyp_boundaries:
            if hyp_start < pos < hyp_start + size:
                projected_hyp_boundaries.add(ref_start + (pos - hyp_start))

    true_positive = len(projected_hyp_boundaries & ref_boundaries)
    precision = true_positive / len(hyp_boundaries)
    recall = true_positive / len(ref_boundaries)
    if precision + recall == 0.0:
        return 0.0
    return 2.0 * precision * recall / (precision + recall)


# ─── Public API ───────────────────────────────────────────────────────────────

def evaluate_text_quality(
    gt: Optional[str],
    pred: Optional[str],
) -> Dict[str, Optional[float]]:
    """Compute text-content quality metrics between GT and prediction Markdown.

    Both inputs are normalised (MD stripped, lowercased) before metric
    computation so that formatting differences do not affect the scores.

    Parameters
    ----------
    gt:   Ground-truth Markdown string.
    pred: Predicted Markdown string.

    Returns
    -------
    dict with keys:
        bleu4              BLEU-4 with smoothing         [0–1]  ↑ better
        rouge1             ROUGE-1 F1                    [0–1]  ↑ better
        rouge2             ROUGE-2 F1                    [0–1]  ↑ better
        rougeL             ROUGE-L F1                    [0–1]  ↑ better
        cer                Character Error Rate          [0–2]  ↓ better
        wer                Word Error Rate               [0–2]  ↓ better
        f1_token           Bag-of-words token F1         [0–1]  ↑ better
        word_fragmentation_score OCR split-word fidelity [0–1]  ↑ better
        word_boundary_integrity_score Preserves whole-word boundaries [0–1]  ↑ better
        token_boundary_f1 Symmetric word-boundary fidelity [0–1]  ↑ better
        text_quality_score mean(rouge1, rougeL, bleu4, word_fragmentation_score, word_boundary_integrity_score, token_boundary_f1)  [0–1]  ↑ better
    """
    gt_plain   = strip_markdown(gt   or "")
    pred_plain = strip_markdown(pred or "")

    _null: Dict[str, Optional[float]] = {
        "bleu4": None, "rouge1": None, "rouge2": None, "rougeL": None,
        "cer": None, "wer": None, "f1_token": None,
        "word_fragmentation_score": None,
        "word_boundary_integrity_score": None,
        "token_boundary_f1": None,
        "text_quality_score": None,
    }

    if not gt_plain:
        return _null

    ref_tokens = _tokenize(gt_plain)
    hyp_tokens = _tokenize(pred_plain)

    bleu4  = _bleu4(ref_tokens, hyp_tokens)
    rouge1 = _rouge_n_f1(ref_tokens, hyp_tokens, 1)
    rouge2 = _rouge_n_f1(ref_tokens, hyp_tokens, 2)
    rouge_l = _rouge_l_f1(ref_tokens, hyp_tokens)
    cer    = _cer(gt_plain, pred_plain)
    wer    = _wer(gt_plain, pred_plain)
    f1_tok = _f1_token(ref_tokens, hyp_tokens)
    word_fragmentation_score = _word_fragmentation_score(ref_tokens, hyp_tokens)
    word_boundary_integrity_score = _word_boundary_integrity_score(ref_tokens, hyp_tokens)
    token_boundary_f1 = _token_boundary_f1(ref_tokens, hyp_tokens)

    # Composite: content fidelity plus explicit split-word corruption penalty.
    quality_parts = [
        v
        for v in (
            rouge1,
            rouge_l,
            bleu4,
            word_fragmentation_score,
            word_boundary_integrity_score,
            token_boundary_f1,
        )
        if v is not None
    ]
    text_quality_score = sum(quality_parts) / len(quality_parts) if quality_parts else None

    return {
        "bleu4":              bleu4,
        "rouge1":             rouge1,
        "rouge2":             rouge2,
        "rougeL":             rouge_l,
        "cer":                cer,
        "wer":                wer,
        "f1_token":           f1_tok,
        "word_fragmentation_score": word_fragmentation_score,
        "word_boundary_integrity_score": word_boundary_integrity_score,
        "token_boundary_f1": token_boundary_f1,
        "text_quality_score": text_quality_score,
    }
