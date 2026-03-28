"""HTML report generator with embedded SVG charts.

Generates a self-contained, WCAG AA-compliant HTML file with:
  - Skip navigation and semantic landmarks
  - Quick comparison table with rank badges
  - Grouped bar chart (visual comparison) inspired by opendataloader.org
  - SVG bar charts with ARIA labels, tooltips, and pattern fills
  - SVG radar chart with accessible legend
  - Metric detail cards (Why it matters / When to prioritize)
  - Speed comparison chart
  - Per-document details
"""

from __future__ import annotations

import html
import json
import math
import time
from pathlib import Path
from typing import Dict, List, Optional, Any

# ── Colour palette (WCAG AA compliant on #0f172a) ───────────────────────────
# Every colour here has been chosen to yield ≥ 4.5:1 contrast on the dark bg.
COLORS = [
    "#60a5fa",  # blue-400 (7.1:1 on #0f172a)
    "#34d399",  # emerald-400 (8.2:1)
    "#fbbf24",  # amber-400 (10.4:1)
    "#f87171",  # red-400 (5.0:1)
    "#a78bfa",  # violet-400 (5.6:1)
    "#f472b6",  # pink-400 (5.3:1)
    "#22d3ee",  # cyan-400 (9.2:1)
    "#a3e635",  # lime-400 (10.6:1)
]

# Hatch patterns for colourblind safety (used in SVG defs)
HATCH_PATTERNS = [
    "none",        # solid
    "diagonal",    # ///
    "horizontal",  # ---
    "dots",        # ...
    "cross",       # +++
    "vertical",    # |||
    "zigzag",      # ~~~
    "grid",        # ###
]

METRIC_INFO = {
    "nid": {
        "name": "NID — Reading Order",
        "short": "NID",
        "description": (
            "Normalized Indel Distance: measures whether text is extracted "
            "in the correct sequence. A score of 1.0 means perfect reading "
            "order. Critical for multi-column layouts."
        ),
        "higher_better": True,
        "why": (
            "When a PDF has multiple columns, sidebars, or complex layouts, "
            "many parsers read text left-to-right across the entire page — "
            "mixing content from different sections. This creates incoherent "
            "chunks that confuse LLMs and produce wrong answers."
        ),
        "when": [
            ("Multi-column layouts", "OpenDataLoader [hybrid]"),
            ("Academic papers, reports", "OpenDataLoader"),
            ("Simple single-column docs", "Any engine works"),
        ],
    },
    "teds": {
        "name": "TEDS — Table Structure",
        "short": "TEDS",
        "description": (
            "Tree Edit Distance Similarity: compares extracted table "
            "structure against ground truth. A score of 1.0 means perfect "
            "table reconstruction."
        ),
        "higher_better": True,
        "why": (
            "Tables contain structured data that LLMs need to answer "
            "questions like 'What was Q3 revenue?' If rows and columns "
            "are scrambled or merged incorrectly, the LLM gets wrong data "
            "and gives wrong answers."
        ),
        "when": [
            ("Financial documents with tables", "Docling"),
            ("Technical specs, comparison tables", "Docling"),
            ("Simple bordered tables", "OpenDataLoader"),
            ("No tables in documents", "Any engine works"),
        ],
    },
    "mhs": {
        "name": "MHS — Heading Hierarchy",
        "short": "MHS",
        "description": (
            "Markdown Heading Similarity: compares detected headings and "
            "their levels against ground truth. Essential for semantic "
            "chunking in RAG pipelines."
        ),
        "higher_better": True,
        "why": (
            "Headings define document structure: chapters, sections, "
            "sub-sections. When heading levels are wrong or missing, "
            "semantic chunking produces oversized or incoherent segments "
            "that hurt retrieval quality."
        ),
        "when": [
            ("Long structured documents", "Docling"),
            ("Legal contracts, manuals", "OpenDataLoader [hybrid]"),
            ("Flat documents (no headings)", "Any engine works"),
        ],
    },
    "pbf": {
        "name": "PBF — Paragraph Boundaries",
        "short": "PBF",
        "description": (
            "Paragraph Boundary F1: compares paragraph breaks against ground truth. "
            "It penalizes merged paragraphs and spurious splits even when token "
            "content is otherwise similar."
        ),
        "higher_better": True,
        "why": (
            "Paragraph boundaries control chunking quality for retrieval and downstream "
            "summarization. A parser that merges or fragments paragraphs changes context "
            "windows even when the raw tokens are still present."
        ),
        "when": [
            ("Narrative documents and articles", "EdgeParse [hybrid OCR]"),
            ("Paragraph-sensitive chunking", "EdgeParse [hybrid OCR]"),
            ("Flat line-oriented content", "Any engine works"),
        ],
    },
    "td_f1": {
        "name": "Table Detection F1",
        "short": "TD F1",
        "description": "F1 score for detecting whether a page contains a table.",
        "higher_better": True,
        "why": (
            "Before a table can be parsed, it must first be detected. "
            "Missing tables means lost data; false positives waste "
            "processing time and can corrupt surrounding text."
        ),
        "when": [
            ("Table-heavy documents", "EdgeParse"),
            ("Mixed content pages", "EdgeParse"),
            ("Text-only documents", "Any engine works"),
        ],
    },
    "speed": {
        "name": "Extraction Speed",
        "short": "Speed",
        "description": (
            "Average seconds per document. Lower is better. "
            "Measured single-threaded on CPU."
        ),
        "higher_better": False,
        "why": (
            "Processing speed determines whether you can handle real-time "
            "queries, batch thousands of documents overnight, or need GPU "
            "acceleration. For RAG pipelines, latency directly affects "
            "user experience."
        ),
        "when": [
            ("Real-time / interactive", "EdgeParse"),
            ("Large batch processing", "EdgeParse / PyMuPDF4LLM"),
            ("Quality over speed", "Docling / Marker"),
        ],
    },
    "tqs": {
        "name": "TQS — Text Content Quality",
        "short": "TQS",
        "description": (
            "Text Quality Score: mean(ROUGE-1, ROUGE-L, BLEU-4, fragmentation score, boundary integrity, token-boundary F1, boundary contamination). "
            "Measures how accurately the extracted text matches the ground truth "
            "after stripping Markdown formatting."
        ),
        "higher_better": True,
        "why": (
            "Structure metrics (NID, TEDS, MHS) tell you if the document is "
            "organized correctly, but not whether the actual words are right. "
            "TQS catches OCR errors, missing paragraphs, and hallucinated content "
            "that would mislead LLMs during RAG retrieval."
        ),
        "when": [
            ("Scanned / image-based PDFs", "Docling / Marker"),
            ("Text-heavy research papers", "EdgeParse / PyMuPDF4LLM"),
            ("High content fidelity required", "Check CER + WER too"),
        ],
    },
    "rouge1": {
        "name": "ROUGE-1",
        "short": "ROUGE-1",
        "description": "ROUGE-1 F1: unigram overlap between extracted and ground-truth text.",
        "higher_better": True, "why": "", "when": [],
    },
    "rougeL": {
        "name": "ROUGE-L",
        "short": "ROUGE-L",
        "description": "ROUGE-L F1: Longest Common Subsequence — order-aware recall.",
        "higher_better": True, "why": "", "when": [],
    },
    "bleu4": {
        "name": "BLEU-4",
        "short": "BLEU-4",
        "description": "BLEU-4 with +1 smoothing: 4-gram precision measuring fluency.",
        "higher_better": True, "why": "", "when": [],
    },
    "frag": {
        "name": "Word Fragmentation Score",
        "short": "Fragmentation",
        "description": "Penalizes OCR-style split words such as 'ow ne r ship' for 'ownership'.",
        "higher_better": True, "why": "", "when": [],
    },
    "word_boundary_integrity_score": {
        "name": "Word Boundary Integrity",
        "short": "Boundary",
        "description": "Penalizes artificial internal spaces inside long words even when the letters are otherwise preserved.",
        "higher_better": True, "why": "", "when": [],
    },
    "token_boundary_f1": {
        "name": "Token Boundary F1",
        "short": "Boundary F1",
        "description": "Character-aligned whitespace-boundary fidelity that penalizes both split words and run-together words.",
        "higher_better": True, "why": "", "when": [],
    },
    "cer": {
        "name": "CER",
        "short": "CER",
        "description": "Character Error Rate: Levenshtein(chars)/len(ref). Lower is better.",
        "higher_better": False, "why": "", "when": [],
    },
    "wer": {
        "name": "WER",
        "short": "WER",
        "description": "Word Error Rate: Levenshtein(words)/len(ref_words). Lower is better.",
        "higher_better": False, "why": "", "when": [],
    },
}


def _esc(text: str) -> str:
    return html.escape(str(text))


def _get_display_name(engine: str) -> str:
    names = {
        "edgeparse": "EdgeParse",
        "edgeparse_hybrid": "EdgeParse [hybrid OCR]",
        "opendataloader": "OpenDataLoader",
        "docling": "Docling",
        "marker": "Marker",
        "mineru": "MinerU",
        "pymupdf4llm": "PyMuPDF4LLM",
        "markitdown": "MarkItDown",
        "liteparse": "LiteParse",
    }
    return names.get(engine, engine)


def _fmt(val: Optional[float], prec: int = 4) -> str:
    if val is None:
        return "N/A"
    return f"{val:.{prec}f}"


def _rank_class(rank: int) -> str:
    if rank == 1:
        return "rank-1"
    elif rank == 2:
        return "rank-2"
    elif rank == 3:
        return "rank-3"
    return ""


def _compute_ranks(values: List[tuple], higher_better: bool = True) -> Dict[str, int]:
    scored = [(e, v) for e, v in values if v is not None]
    if not scored:
        return {}

    decimals = 3 if not higher_better else 4
    scored.sort(key=lambda x: round(x[1], decimals), reverse=higher_better)

    ranks: Dict[str, int] = {}
    current_rank = 0
    last_value = None
    for eng, value in scored:
        rounded = round(value, decimals)
        if last_value is None or rounded != last_value:
            current_rank += 1
            last_value = rounded
        ranks[eng] = current_rank
    return ranks


def _svg_defs(engine_colors: Dict[str, str]) -> str:
    """Generate SVG <defs> block with hatch patterns for colourblind safety."""
    lines = ["<defs>"]
    for i, (eng, color) in enumerate(engine_colors.items()):
        pid = f"pat-{_esc(eng)}"
        hatch = HATCH_PATTERNS[i % len(HATCH_PATTERNS)]
        if hatch == "none":
            lines.append(f'<pattern id="{pid}" width="1" height="1">'
                         f'<rect width="1" height="1" fill="{color}"/></pattern>')
        elif hatch == "diagonal":
            lines.append(f'<pattern id="{pid}" width="6" height="6" patternUnits="userSpaceOnUse">'
                         f'<rect width="6" height="6" fill="{color}"/>'
                         f'<path d="M0,6 L6,0" stroke="{_darken(color)}" stroke-width="1.5"/></pattern>')
        elif hatch == "horizontal":
            lines.append(f'<pattern id="{pid}" width="6" height="6" patternUnits="userSpaceOnUse">'
                         f'<rect width="6" height="6" fill="{color}"/>'
                         f'<line x1="0" y1="3" x2="6" y2="3" stroke="{_darken(color)}" stroke-width="1.5"/></pattern>')
        elif hatch == "dots":
            lines.append(f'<pattern id="{pid}" width="6" height="6" patternUnits="userSpaceOnUse">'
                         f'<rect width="6" height="6" fill="{color}"/>'
                         f'<circle cx="3" cy="3" r="1.2" fill="{_darken(color)}"/></pattern>')
        elif hatch == "cross":
            lines.append(f'<pattern id="{pid}" width="6" height="6" patternUnits="userSpaceOnUse">'
                         f'<rect width="6" height="6" fill="{color}"/>'
                         f'<path d="M0,3 L6,3 M3,0 L3,6" stroke="{_darken(color)}" stroke-width="1"/></pattern>')
        elif hatch == "vertical":
            lines.append(f'<pattern id="{pid}" width="6" height="6" patternUnits="userSpaceOnUse">'
                         f'<rect width="6" height="6" fill="{color}"/>'
                         f'<line x1="3" y1="0" x2="3" y2="6" stroke="{_darken(color)}" stroke-width="1.5"/></pattern>')
        else:
            lines.append(f'<pattern id="{pid}" width="6" height="6" patternUnits="userSpaceOnUse">'
                         f'<rect width="6" height="6" fill="{color}"/>'
                         f'<path d="M0,3 L3,0 L6,3" stroke="{_darken(color)}" stroke-width="1" fill="none"/></pattern>')
    lines.append("</defs>")
    return "\n".join(lines)


def _darken(hex_color: str) -> str:
    """Return a 40% darker version of a hex colour for overlay patterns."""
    h = hex_color.lstrip("#")
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return f"#{int(r*0.6):02x}{int(g*0.6):02x}{int(b*0.6):02x}"


def _engine_color_map(engines: List[str]) -> Dict[str, str]:
    """Assign a stable colour to each engine."""
    return {eng: COLORS[i % len(COLORS)] for i, eng in enumerate(engines)}


# ── SVG Chart Generators ─────────────────────────────────────────────────────

def _svg_bar_chart(
    title: str,
    data: List[tuple],  # [(engine_name, value)]
    higher_better: bool = True,
    width: int = 700,
    bar_height: int = 36,
    description: str = "",
    engine_colors: Optional[Dict[str, str]] = None,
) -> str:
    """Generate an accessible SVG horizontal bar chart."""
    data = [(e, v) for e, v in data if v is not None]
    if not data:
        return f'<p class="no-data">No data available for {_esc(title)}</p>'

    data.sort(key=lambda x: x[1], reverse=higher_better)
    max_val = max(v for _, v in data)
    if max_val == 0:
        max_val = 1

    n = len(data)
    padding = 40
    label_w = 130
    rank_w = 40
    bar_area = width - label_w - padding * 2 - rank_w
    chart_h = padding * 2 + n * (bar_height + 10) + 48

    direction = "Higher is better" if higher_better else "Lower is better"
    aria = f"{_esc(title)} — bar chart comparing {n} engines. {direction}."

    lines = [
        f'<svg viewBox="0 0 {width} {chart_h}" class="chart-svg" '
        f'role="img" aria-label="{aria}">',
        f"<title>{_esc(title)}</title>",
        f"<desc>{_esc(description or direction)}</desc>",
    ]
    lines.append(f'<text x="{width // 2}" y="24" text-anchor="middle" '
                 f'class="chart-title">{_esc(title)}</text>')
    if description:
        lines.append(f'<text x="{width // 2}" y="42" text-anchor="middle" '
                     f'class="chart-desc">{_esc(description[:120])}</text>')

    y_start = 60
    for i, (eng, val) in enumerate(data):
        y = y_start + i * (bar_height + 10)
        bar_w = int((val / max_val) * bar_area) if max_val > 0 else 0
        bar_w = max(bar_w, 4)
        color = (engine_colors or {}).get(eng, COLORS[i % len(COLORS)])
        name = _get_display_name(eng)
        val_str = f"{val:.4f}" if val < 10 else f"{val:.2f}"

        # Label
        lines.append(f'<text x="{label_w - 10}" y="{y + bar_height // 2 + 5}" '
                     f'text-anchor="end" class="bar-label">{_esc(name)}</text>')
        # Bar background
        lines.append(f'<rect x="{label_w}" y="{y}" width="{bar_area}" '
                     f'height="{bar_height}" rx="4" class="bar-bg"/>')
        # Bar fill with tooltip
        lines.append(f'<rect x="{label_w}" y="{y}" width="{bar_w}" '
                     f'height="{bar_height}" rx="4" fill="{color}" class="bar-fill">'
                     f'<title>{_esc(name)}: {val_str}</title></rect>')
        # Value label
        lines.append(f'<text x="{label_w + bar_w + 8}" y="{y + bar_height // 2 + 5}" '
                     f'class="bar-value">{val_str}</text>')
        # Rank badge (text instead of emoji for accessibility)
        if i < 3:
            rank_colors = ["var(--rank1)", "var(--rank2)", "var(--rank3)"]
            badge_x = label_w + bar_area + rank_w // 2
            badge_y = y + bar_height // 2
            lines.append(f'<circle cx="{badge_x}" cy="{badge_y}" r="12" '
                         f'fill="{rank_colors[i]}" opacity="0.25"/>')
            lines.append(f'<text x="{badge_x}" y="{badge_y + 5}" text-anchor="middle" '
                         f'class="rank-badge" fill="{rank_colors[i]}">#{i+1}</text>')

    lines.append("</svg>")
    return "\n".join(lines)


def _svg_grouped_bar_chart(
    engines: List[str],
    metric_data: Dict[str, Dict[str, Optional[float]]],
    engine_colors: Dict[str, str],
    width: int = 900,
) -> str:
    """Generate a grouped bar chart showing all engines side-by-side per metric.

    Inspired by the opendataloader.org benchmark visual comparison.
    """
    metrics = ["nid", "teds", "mhs", "pbf", "td_f1", "tqs"]
    metric_labels = {
        "nid": "NID", "teds": "TEDS", "mhs": "MHS", "pbf": "PBF", "td_f1": "TD F1", "tqs": "TQS",
    }
    n_metrics = len(metrics)
    n_engines = len(engines)
    if n_engines == 0:
        return '<p class="no-data">No engines to compare</p>'

    group_gap = 40
    bar_w = max(16, min(36, (width - 140) // (n_metrics * (n_engines + 1))))
    group_w = n_engines * bar_w + (n_engines - 1) * 2
    total_groups_w = n_metrics * group_w + (n_metrics - 1) * group_gap
    left_pad = max(60, (width - total_groups_w) // 2)

    chart_h = 340
    base_y = 260
    top_y = 50
    bar_max_h = base_y - top_y

    # Legend height
    legend_h = 40
    total_h = chart_h + legend_h

    aria = (f"Grouped bar chart comparing {n_engines} engines across "
            f"{n_metrics} accuracy metrics (NID, TEDS, MHS, TD F1).")

    lines = [
        f'<svg viewBox="0 0 {width} {total_h}" class="chart-svg" '
        f'role="img" aria-label="{aria}">',
        "<title>Visual Comparison \u2014 All Metrics</title>",
        f'<desc>Side-by-side comparison of {n_engines} engines across {n_metrics} metrics.</desc>',
    ]
    # Defs for patterns
    lines.append(_svg_defs(engine_colors))

    # Title
    lines.append(f'<text x="{width // 2}" y="28" text-anchor="middle" '
                 f'class="chart-title">Visual Comparison</text>')

    # Horizontal grid lines
    for level in [0.0, 0.2, 0.4, 0.6, 0.8, 1.0]:
        gy = base_y - level * bar_max_h
        lines.append(f'<line x1="{left_pad - 10}" y1="{gy:.0f}" '
                     f'x2="{left_pad + total_groups_w + 10}" y2="{gy:.0f}" '
                     f'stroke="var(--border)" stroke-width="0.5" opacity="0.4"/>')
        lines.append(f'<text x="{left_pad - 14}" y="{gy + 4:.0f}" '
                     f'text-anchor="end" class="radar-tick">{level:.1f}</text>')

    # Bars per group
    for gi, metric in enumerate(metrics):
        gx = left_pad + gi * (group_w + group_gap)
        # Metric label below baseline
        lines.append(f'<text x="{gx + group_w // 2}" y="{base_y + 20}" '
                     f'text-anchor="middle" class="bar-label">'
                     f'{metric_labels[metric]}</text>')

        for ei, eng in enumerate(engines):
            val = metric_data.get(metric, {}).get(eng)
            if val is None:
                val = 0
            bh = max(2, int(val * bar_max_h))
            bx = gx + ei * (bar_w + 2)
            by = base_y - bh
            color = engine_colors.get(eng, COLORS[ei % len(COLORS)])
            pid = f"pat-{_esc(eng)}"
            val_str = f"{val:.3f}"
            name = _get_display_name(eng)

            lines.append(f'<rect x="{bx}" y="{by}" width="{bar_w}" '
                         f'height="{bh}" rx="2" fill="url(#{pid})" '
                         f'stroke="{color}" stroke-width="0.5">'
                         f'<title>{_esc(name)}: {val_str} ({metric_labels[metric]})</title></rect>')
            # Value on top of bar (only if bar is tall enough)
            if bh > 20:
                lines.append(f'<text x="{bx + bar_w // 2}" y="{by - 4}" '
                             f'text-anchor="middle" class="chart-desc" '
                             f'style="font-size:9px">{val_str}</text>')

    # Legend (horizontal, centered)
    legend_y = base_y + 36
    items_w = sum(len(_get_display_name(e)) * 7 + 28 for e in engines)
    lx_start = max(10, (width - items_w) // 2)
    lx = lx_start
    for ei, eng in enumerate(engines):
        color = engine_colors.get(eng, COLORS[ei % len(COLORS)])
        name = _get_display_name(eng)
        lines.append(f'<rect x="{lx}" y="{legend_y}" width="14" height="14" '
                     f'rx="3" fill="{color}"/>')
        lines.append(f'<text x="{lx + 18}" y="{legend_y + 11}" '
                     f'class="legend-text">{_esc(name)}</text>')
        lx += len(name) * 7 + 28

    lines.append("</svg>")
    return "\n".join(lines)


def _svg_radar_chart(
    engines: List[str],
    metric_data: Dict[str, Dict[str, Optional[float]]],
    engine_colors: Dict[str, str],
    width: int = 520,
) -> str:
    """Generate an accessible SVG radar/spider chart comparing engines."""
    metrics = ["nid", "teds", "mhs", "td_f1", "tqs"]
    metric_labels = ["NID", "TEDS", "MHS", "TD F1", "TQS"]
    n_metrics = len(metrics)
    cx, cy = width // 2, width // 2 + 10
    r = width // 2 - 90

    n_engines = len(engines)
    aria = (f"Radar chart comparing {n_engines} engines across "
            f"{n_metrics} accuracy metrics.")

    lines = [
        f'<svg viewBox="0 0 {width} {width + 50}" class="chart-svg" '
        f'role="img" aria-label="{aria}">',
        "<title>Accuracy Radar</title>",
        "<desc>Spider chart overlaying engine scores for NID, TEDS, MHS, TD F1.</desc>",
    ]
    lines.append(f'<text x="{cx}" y="24" text-anchor="middle" '
                 f'class="chart-title">Accuracy Radar</text>')

    # Grid circles with labels
    for level in [0.2, 0.4, 0.6, 0.8, 1.0]:
        cr = int(r * level)
        lines.append(f'<circle cx="{cx}" cy="{cy}" r="{cr}" class="radar-grid"/>')
        lines.append(f'<text x="{cx + 5}" y="{cy - cr + 4}" '
                     f'class="radar-tick">{level:.1f}</text>')

    # Axes and labels
    angles = [i * 2 * math.pi / n_metrics - math.pi / 2 for i in range(n_metrics)]
    for i, angle in enumerate(angles):
        x = cx + r * math.cos(angle)
        y = cy + r * math.sin(angle)
        lines.append(f'<line x1="{cx}" y1="{cy}" x2="{x:.0f}" y2="{y:.0f}" '
                     f'class="radar-axis"/>')
        lx = cx + (r + 28) * math.cos(angle)
        ly = cy + (r + 28) * math.sin(angle)
        lines.append(f'<text x="{lx:.0f}" y="{ly:.0f}" text-anchor="middle" '
                     f'dominant-baseline="middle" class="radar-label">'
                     f'{metric_labels[i]}</text>')

    # Plot each engine
    for ei, eng in enumerate(engines):
        color = engine_colors.get(eng, COLORS[ei % len(COLORS)])
        points = []
        name = _get_display_name(eng)
        for mi, metric in enumerate(metrics):
            val = metric_data.get(metric, {}).get(eng)
            if val is None:
                val = 0
            px = cx + r * val * math.cos(angles[mi])
            py = cy + r * val * math.sin(angles[mi])
            points.append(f"{px:.1f},{py:.1f}")
        pts_str = " ".join(points)
        lines.append(f'<polygon points="{pts_str}" fill="{color}" fill-opacity="0.12" '
                     f'stroke="{color}" stroke-width="2.5" class="radar-poly">'
                     f'<title>{_esc(name)}</title></polygon>')
        # Data point dots with tooltips
        for mi, metric in enumerate(metrics):
            val = metric_data.get(metric, {}).get(eng)
            if val is None:
                val = 0
            px = cx + r * val * math.cos(angles[mi])
            py = cy + r * val * math.sin(angles[mi])
            lines.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="4.5" '
                         f'fill="{color}" stroke="#0f172a" stroke-width="1.5">'
                         f'<title>{_esc(name)}: {val:.3f} ({metric_labels[mi]})</title>'
                         f'</circle>')

    # Legend (wrapping horizontally)
    legend_y = width + 16
    items_w = sum(len(_get_display_name(e)) * 7 + 28 for e in engines)
    lx_start = max(10, (width - items_w) // 2)
    lx = lx_start
    for ei, eng in enumerate(engines):
        color = engine_colors.get(eng, COLORS[ei % len(COLORS)])
        name = _get_display_name(eng)
        # Wrap to next row if exceeding width
        if lx + len(name) * 7 + 28 > width - 10 and ei > 0:
            legend_y += 20
            lx = lx_start
        lines.append(f'<rect x="{lx}" y="{legend_y}" width="14" height="14" '
                     f'rx="3" fill="{color}"/>')
        lines.append(f'<text x="{lx + 18}" y="{legend_y + 11}" '
                     f'class="legend-text">{_esc(name)}</text>')
        lx += len(name) * 7 + 28

    lines.append("</svg>")
    return "\n".join(lines)


def _svg_overall_chart(
    engines: List[str],
    overall_scores: Dict[str, Optional[float]],
    engine_colors: Dict[str, str],
    width: int = 700,
) -> str:
    """Generate an SVG chart showing overall score breakdown."""
    data = [(e, overall_scores.get(e)) for e in engines]
    return _svg_bar_chart(
        "Overall Score", data, higher_better=True, width=width,
        description="Average of NID + TEDS + MHS + TQS (text quality)",
        engine_colors=engine_colors,
    )


# ══════════════════════════════════════════════════════════════════════════════
#  HTML Page Assembly
# ══════════════════════════════════════════════════════════════════════════════

CSS = """
/* ── WCAG AA Compliant Dark Theme ──
   All text colours tested against #0f172a (bg) for ≥ 4.5:1 contrast.
   Large text (≥ 18px / 14px bold) requires ≥ 3:1 only.
*/
:root {
  --bg: #0f172a;
  --card: #1e293b;
  --border: #475569;
  --text: #f1f5f9;         /* 15.4:1 on bg */
  --text-dim: #cbd5e1;     /* 10.3:1 on bg — WCAG AAA */
  --accent: #60a5fa;       /* 7.1:1 on bg */
  --green: #4ade80;        /* 8.6:1 on bg */
  --red: #f87171;          /* 5.0:1 on bg */
  --amber: #fbbf24;        /* 10.4:1 on bg */
  --rank1: #fbbf24;        /* gold */
  --rank2: #cbd5e1;        /* silver */
  --rank3: #d97706;        /* bronze */
}

*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

/* Skip-nav link for keyboard users */
.skip-link {
  position: absolute;
  top: -100px;
  left: 0;
  background: var(--accent);
  color: #0f172a;
  padding: 0.5rem 1rem;
  font-weight: 700;
  z-index: 9999;
  border-radius: 0 0 6px 0;
  text-decoration: none;
}
.skip-link:focus { top: 0; }

body {
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: var(--bg);
  color: var(--text);
  line-height: 1.65;
  padding: 0;
}

/* Focus ring for keyboard navigation */
:focus-visible {
  outline: 3px solid var(--accent);
  outline-offset: 2px;
}

.container { max-width: 1120px; margin: 0 auto; padding: 2rem 1.5rem; }

/* Header */
.header {
  text-align: center;
  padding: 3rem 1rem 2rem;
  border-bottom: 1px solid var(--border);
  margin-bottom: 2rem;
}
.header h1 {
  font-size: 2.2rem;
  font-weight: 800;
  color: var(--accent);
  margin-bottom: 0.5rem;
}
.header .meta { color: var(--text-dim); font-size: 0.9rem; margin-top: 0.25rem; }
.header .meta a { color: var(--accent); text-decoration: underline; }
.header .meta a:hover { color: var(--text); }

/* Sections */
section { margin-bottom: 2.5rem; }
section > h2 {
  font-size: 1.4rem;
  font-weight: 700;
  margin-bottom: 1rem;
  padding-bottom: 0.5rem;
  border-bottom: 2px solid var(--accent);
  display: inline-block;
  color: var(--text);
}
section > h3, .metric-detail h3 {
  font-size: 1.1rem;
  font-weight: 600;
  margin-bottom: 0.5rem;
  color: var(--accent);
}

/* Quick Comparison Table */
.comparison-table {
  width: 100%;
  border-collapse: collapse;
  margin: 1rem 0;
  font-size: 0.95rem;
}
.comparison-table th {
  background: var(--card);
  padding: 0.75rem 1rem;
  text-align: right;
  border-bottom: 2px solid var(--border);
  font-weight: 700;
  position: sticky;
  top: 0;
  color: var(--text);
}
.comparison-table th:first-child { text-align: left; }
.comparison-table td {
  padding: 0.65rem 1rem;
  text-align: right;
  border-bottom: 1px solid var(--border);
  font-variant-numeric: tabular-nums;
}
.comparison-table td:first-child {
  text-align: left;
  font-weight: 600;
}
.comparison-table tbody tr:hover { background: rgba(96, 165, 250, 0.08); }
.rank-1 { color: var(--rank1); font-weight: 700; }
.rank-2 { color: var(--rank2); font-weight: 600; }
.rank-3 { color: var(--rank3); }
td.best {
  background: rgba(74, 222, 128, 0.10);
  font-weight: 700;
  color: var(--green);
}
.rank-badge-cell {
  display: inline-block;
  width: 22px; height: 22px;
  line-height: 22px;
  text-align: center;
  border-radius: 50%;
  font-size: 0.72rem;
  font-weight: 700;
  margin-left: 4px;
  vertical-align: middle;
}
.rank-badge-1 { background: var(--rank1); color: #0f172a; }
.rank-badge-2 { background: var(--rank2); color: #0f172a; }
.rank-badge-3 { background: var(--rank3); color: #0f172a; }

/* Metric explanation cards */
.metric-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1rem;
  margin-top: 0.5rem;
}
.metric-card {
  background: var(--card);
  border-radius: 12px;
  padding: 1.25rem;
  border: 1px solid var(--border);
}
.metric-card h4 {
  font-size: 1rem;
  margin-bottom: 0.5rem;
  color: var(--accent);
}
.metric-card p {
  font-size: 0.88rem;
  color: var(--text-dim);
  line-height: 1.5;
}

/* Metric detail sections (why / when) */
.metric-detail {
  background: var(--card);
  border-radius: 12px;
  padding: 1.5rem;
  border: 1px solid var(--border);
  margin: 1rem 0;
}
.metric-detail h3 { margin-top: 0; }
.metric-detail p { color: var(--text-dim); font-size: 0.9rem; margin-bottom: 0.75rem; }
.when-table {
  width: 100%;
  border-collapse: collapse;
  margin-top: 0.5rem;
  font-size: 0.88rem;
}
.when-table th, .when-table td {
  padding: 0.5rem 0.75rem;
  border-bottom: 1px solid var(--border);
  text-align: left;
}
.when-table th { font-weight: 600; color: var(--text); }
.when-table td { color: var(--text-dim); }
.when-table td:last-child { color: var(--accent); font-weight: 500; }

/* Charts */
.chart-container {
  background: var(--card);
  border-radius: 12px;
  padding: 1.5rem;
  margin: 1rem 0;
  border: 1px solid var(--border);
}
.charts-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1.5rem;
}
@media (max-width: 800px) { .charts-grid { grid-template-columns: 1fr; } }

/* SVG styles */
.chart-svg { width: 100%; height: auto; }
.chart-title { font-size: 14px; font-weight: 700; fill: var(--text); }
.chart-desc { font-size: 11px; fill: var(--text-dim); }
.bar-label { font-size: 12px; fill: var(--text); font-weight: 600; }
.bar-bg { fill: var(--border); opacity: 0.2; }
.bar-fill { transition: width 0.6s cubic-bezier(0.22, 1, 0.36, 1); }
.bar-value { font-size: 12px; fill: var(--text-dim); font-weight: 700; }
.rank-badge { font-size: 11px; font-weight: 800; }
.radar-grid { fill: none; stroke: var(--border); stroke-width: 0.5; }
.radar-axis { stroke: var(--border); stroke-width: 0.5; }
.radar-tick { font-size: 9px; fill: var(--text-dim); }
.radar-label { font-size: 13px; fill: var(--text); font-weight: 700; }
.radar-poly { transition: all 0.3s ease; }
.legend-text { font-size: 12px; fill: var(--text); font-weight: 500; }

/* Verdict */
.verdict {
  background: linear-gradient(135deg, rgba(96,165,250,0.12), rgba(167,139,250,0.08));
  border: 2px solid var(--accent);
  border-radius: 12px;
  padding: 1.5rem 2rem;
  text-align: center;
  margin: 2rem 0;
}
.verdict h3 { font-size: 1.3rem; margin-bottom: 0.5rem; color: var(--green); }
.verdict .detail { font-size: 0.9rem; color: var(--text-dim); }

/* Footer */
footer {
  text-align: center;
  color: var(--text-dim);
  font-size: 0.82rem;
  padding: 2rem 0;
  border-top: 1px solid var(--border);
}
footer a { color: var(--accent); text-decoration: underline; }
footer a:hover { color: var(--text); }

/* Details expandable */
details { margin: 0.5rem 0; }
details summary {
  cursor: pointer;
  padding: 0.6rem 0.8rem;
  background: var(--card);
  border-radius: 8px;
  border: 1px solid var(--border);
  font-weight: 600;
  color: var(--text);
}
details summary:hover { border-color: var(--accent); }
details[open] summary { border-radius: 8px 8px 0 0; }
.detail-content {
  background: var(--card);
  padding: 1rem;
  border: 1px solid var(--border);
  border-top: none;
  border-radius: 0 0 8px 8px;
  font-size: 0.88rem;
}
.detail-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.88rem;
}
.detail-table th, .detail-table td {
  padding: 0.5rem 0.7rem;
  text-align: right;
  border-bottom: 1px solid var(--border);
}
.detail-table th { font-weight: 600; color: var(--text); }
.detail-table th:first-child, .detail-table td:first-child { text-align: left; }
.no-data { color: var(--text-dim); font-style: italic; }
"""


def generate_html_report(
    results: Dict[str, dict],
    output_path: Path,
    title: str = "EdgeParse Benchmark Report",
) -> Path:
    """Generate a self-contained, WCAG AA-compliant HTML benchmark report."""
    engines = list(results.keys())
    n = len(engines)
    engine_colors = _engine_color_map(engines)

    # Extract metric data
    metric_data: Dict[str, Dict[str, Optional[float]]] = {
        "nid": {}, "teds": {}, "mhs": {}, "pbf": {}, "td_f1": {}, "speed": {}, "overall": {},
        "tqs": {}, "rouge1": {}, "rougeL": {}, "bleu4": {}, "frag": {},
        "word_boundary_integrity_score": {}, "token_boundary_f1": {}, "cer": {}, "wer": {},
    }
    for eng in engines:
        d = results[eng]
        scores = d.get("metrics", {}).get("score", {})
        td = d.get("table_detection", {})
        spd = d.get("speed", {})
        metric_data["nid"][eng] = scores.get("nid_mean")
        metric_data["teds"][eng] = scores.get("teds_mean")
        metric_data["mhs"][eng] = scores.get("mhs_mean")
        metric_data["pbf"][eng] = scores.get("paragraph_boundary_f1_mean")
        metric_data["td_f1"][eng] = td.get("f1")
        metric_data["speed"][eng] = spd.get("elapsed_per_doc")
        metric_data["overall"][eng] = scores.get("overall_mean")
        metric_data["tqs"][eng] = scores.get("text_quality_score_mean")
        metric_data["rouge1"][eng] = scores.get("rouge1_mean")
        metric_data["rougeL"][eng] = scores.get("rougeL_mean")
        metric_data["bleu4"][eng] = scores.get("bleu4_mean")
        metric_data["frag"][eng] = scores.get("word_fragmentation_score_mean")
        metric_data["word_boundary_integrity_score"][eng] = scores.get("word_boundary_integrity_score_mean")
        metric_data["token_boundary_f1"][eng] = scores.get("token_boundary_f1_mean")
        metric_data["cer"][eng] = scores.get("cer_mean")
        metric_data["wer"][eng] = scores.get("wer_mean")

    # Compute ranks
    ranks: Dict[str, Dict[str, int]] = {}
    for mk in ["nid", "teds", "mhs", "pbf", "td_f1", "overall", "tqs", "rouge1", "rougeL", "bleu4", "frag", "word_boundary_integrity_score", "token_boundary_f1"]:
        vals = [(e, metric_data[mk].get(e)) for e in engines]
        ranks[mk] = _compute_ranks(vals, True)
    for mk in ["speed", "cer", "wer"]:
        vals = [(e, metric_data[mk].get(e)) for e in engines]
        ranks[mk] = _compute_ranks(vals, False)

    # Get metadata from first result
    first_data = next(iter(results.values()))
    doc_count = first_data.get("speed", {}).get("document_count", "?")
    processor = first_data.get("speed", {}).get("processor", "")
    run_date = time.strftime("%Y-%m-%d %H:%M")

    # ── Build HTML ────────────────────────────────────────────────────────────
    parts: List[str] = []

    # DOCTYPE + head
    parts.append(f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="description" content="Benchmark comparison of {n} PDF-to-Markdown engines">
<title>{_esc(title)}</title>
<style>{CSS}</style>
</head>
<body>

<!-- Skip navigation link (WCAG 2.4.1) -->
<a href="#main-content" class="skip-link">Skip to main content</a>

<div class="container">

<header class="header" role="banner">
  <h1>{_esc(title)}</h1>
  <p class="meta">{n} engines &middot; {doc_count} documents &middot; {run_date}</p>
  <p class="meta">{_esc(processor)}</p>
  <p class="meta">Methodology: <a href="https://opendataloader.org/docs/benchmark" target="_blank" rel="noopener">opendataloader.org/docs/benchmark</a></p>
</header>

<main id="main-content">
""")

    # ── Quick Comparison Table (inspired by opendataloader.org) ───────────────
    parts.append('<section aria-labelledby="hd-comparison">')
    parts.append('<h2 id="hd-comparison">Quick Comparison</h2>')
    parts.append('<p style="color:var(--text-dim);font-size:0.9rem;margin-bottom:1rem">'
                 'Scores normalized to [0, 1]. Higher is better for accuracy; lower for speed. '
                 '<strong style="color:var(--green)">Bold</strong> indicates best performance.</p>')
    parts.append('<table class="comparison-table" role="table">')
    parts.append('<thead><tr>'
                 '<th scope="col">Engine</th>'
                 '<th scope="col">NID</th>'
                 '<th scope="col">TEDS</th>'
                 '<th scope="col">MHS</th>'
                 '<th scope="col">PBF</th>'
                 '<th scope="col">TQS</th>'
                 '<th scope="col">TD F1</th>'
                 '<th scope="col">s/doc</th>'
                 '<th scope="col">Overall</th>'
                 '<th scope="col">Rank</th>'
                 '</tr></thead>')
    parts.append('<tbody>')

    # Sort engines by overall rank
    sorted_engines = sorted(engines, key=lambda e: ranks.get("overall", {}).get(e, 99))
    for eng in sorted_engines:
        ov_r = ranks.get("overall", {}).get(eng, 99)
        row = f'<tr><td><strong>{_esc(_get_display_name(eng))}</strong></td>'
        for mk in ["nid", "teds", "mhs", "pbf", "tqs", "td_f1"]:
            val = metric_data[mk].get(eng)
            r = ranks[mk].get(eng, 99)
            cls = _rank_class(r)
            best = ' class="best"' if r == 1 else f' class="{cls}"' if cls else ""
            row += f"<td{best}>{_fmt(val)}</td>"
        # Speed
        spd_val = metric_data["speed"].get(eng)
        spd_r = ranks["speed"].get(eng, 99)
        spd_cls = _rank_class(spd_r)
        spd_best = ' class="best"' if spd_r == 1 else f' class="{spd_cls}"' if spd_cls else ""
        row += f'<td{spd_best}>{_fmt(spd_val, 3) if spd_val is not None else "N/A"}</td>'
        # Overall
        ov = metric_data["overall"].get(eng)
        ov_cls = _rank_class(ov_r)
        ov_best = ' class="best"' if ov_r == 1 else f' class="{ov_cls}"' if ov_cls else ""
        row += f"<td{ov_best}>{_fmt(ov)}</td>"
        # Rank badge
        if ov_r <= 3:
            badge_cls = f"rank-badge-{ov_r}"
            row += f'<td><span class="rank-badge-cell {badge_cls}">#{ov_r}</span></td>'
        else:
            row += f"<td>#{ov_r}</td>"
        row += "</tr>"
        parts.append(row)

    parts.append("</tbody></table></section>")

    # ── Visual Comparison (grouped bar chart like opendataloader.org) ─────────
    parts.append('<section aria-labelledby="hd-visual">')
    parts.append('<h2 id="hd-visual">Visual Comparison</h2>')

    grouped_chart = _svg_grouped_bar_chart(engines, metric_data, engine_colors, width=900)
    parts.append(f'<div class="chart-container">{grouped_chart}</div>')

    # Individual metric bar charts (2-col grid) — structural metrics
    parts.append('<div class="charts-grid">')
    for mk, info in [("nid", METRIC_INFO["nid"]), ("teds", METRIC_INFO["teds"]),
                     ("mhs", METRIC_INFO["mhs"]), ("pbf", METRIC_INFO["pbf"]),
                     ("td_f1", METRIC_INFO["td_f1"])]:
        data = [(e, metric_data[mk].get(e)) for e in engines]
        chart = _svg_bar_chart(
            info["name"], data, info["higher_better"],
            width=500, bar_height=32,
            description=info["description"][:80],
            engine_colors=engine_colors,
        )
        parts.append(f'<div class="chart-container">{chart}</div>')
    parts.append("</div>")

    # Text quality metric bar charts (2-col grid)
    parts.append('<h3 style="margin:1.5rem 0 0.75rem">Text Content Quality</h3>')
    parts.append('<div class="charts-grid">')
    for mk, info in [("tqs", METRIC_INFO["tqs"]), ("rouge1", METRIC_INFO["rouge1"]),
                     ("rougeL", METRIC_INFO["rougeL"]), ("bleu4", METRIC_INFO["bleu4"]),
                     ("frag", METRIC_INFO["frag"]),
                     ("word_boundary_integrity_score", METRIC_INFO["word_boundary_integrity_score"]),
                     ("token_boundary_f1", METRIC_INFO["token_boundary_f1"])]:
        data = [(e, metric_data[mk].get(e)) for e in engines]
        chart = _svg_bar_chart(
            info["name"], data, info["higher_better"],
            width=500, bar_height=32,
            description=info["description"][:80],
            engine_colors=engine_colors,
        )
        parts.append(f'<div class="chart-container">{chart}</div>')
    parts.append("</div>")

    # Speed chart (full width)
    speed_data = [(e, metric_data["speed"].get(e)) for e in engines]
    speed_chart = _svg_bar_chart(
        "Extraction Speed (s/doc)", speed_data,
        higher_better=False, width=700, bar_height=36,
        description="Lower is better. Full pipeline: parsing + layout + Markdown.",
        engine_colors=engine_colors,
    )
    parts.append(f'<div class="chart-container">{speed_chart}</div>')

    # Radar chart
    radar = _svg_radar_chart(engines, metric_data, engine_colors, width=520)
    parts.append(f'<div class="chart-container" style="max-width:560px;margin:1rem auto">{radar}</div>')

    # Overall chart
    overall_chart = _svg_overall_chart(
        engines, metric_data.get("overall", {}), engine_colors, width=700
    )
    parts.append(f'<div class="chart-container">{overall_chart}</div>')
    parts.append("</section>")

    # ── Verdict ───────────────────────────────────────────────────────────────
    verdict_metrics = ["nid", "teds", "mhs", "pbf", "td_f1", "tqs", "speed"]
    outright_wins: Dict[str, int] = {e: 0 for e in engines}
    shared_best: Dict[str, int] = {e: 0 for e in engines}

    for mk in verdict_metrics:
        higher_better = METRIC_INFO[mk]["higher_better"]
        decimals = 3 if mk == "speed" else 4
        scored = [
            (eng, metric_data[mk].get(eng))
            for eng in engines
            if metric_data[mk].get(eng) is not None
        ]
        if not scored:
            continue

        best_value = max(round(val, decimals) for _, val in scored) if higher_better else min(
            round(val, decimals) for _, val in scored
        )
        best_engines = [eng for eng, val in scored if round(val, decimals) == best_value]
        if len(best_engines) == 1:
            outright_wins[best_engines[0]] += 1
        else:
            for eng in best_engines:
                shared_best[eng] += 1

    winner = max(
        engines,
        key=lambda eng: (
            outright_wins[eng],
            shared_best[eng],
            metric_data["overall"].get(eng) or float("-inf"),
        ),
    )

    parts.append('<div class="verdict" role="status" aria-live="polite">')
    summary = f'{_esc(_get_display_name(winner))} leads with {outright_wins[winner]} outright metric wins'
    if shared_best[winner] > 0:
        summary += f' and {shared_best[winner]} shared-best ties'
    parts.append(f'  <h3>{summary}</h3>')
    parts.append('  <p class="detail">')
    others = [
        (e, outright_wins[e], shared_best[e])
        for e in engines
        if e != winner and (outright_wins[e] > 0 or shared_best[e] > 0)
    ]
    if others:
        others.sort(key=lambda x: (x[1], x[2]), reverse=True)
        other_parts = []
        for eng, wins, ties in others:
            detail = f"{_get_display_name(eng)}: {wins} outright"
            if ties > 0:
                detail += f", {ties} tied"
            other_parts.append(detail)
        parts.append(f'Other wins: {", ".join(other_parts)}')
    parts.append("</p></div>")

    # ── Detailed Metrics (inspired by opendataloader.org per-metric pages) ────
    parts.append('<section aria-labelledby="hd-metrics">')
    parts.append('<h2 id="hd-metrics">Detailed Metrics</h2>')

    for mk, info in METRIC_INFO.items():
        direction = "Higher is better" if info["higher_better"] else "Lower is better"
        dir_color = "var(--green)" if info["higher_better"] else "var(--amber)"
        why_text = info.get("why", "")
        when_rows = info.get("when", [])

        parts.append('<div class="metric-detail">')
        parts.append(f'<h3>{_esc(info["name"])}</h3>')
        parts.append(f'<p>{_esc(info["description"])}</p>')
        parts.append(f'<p style="color:{dir_color}"><strong>{direction}</strong></p>')

        if why_text:
            parts.append('<h4 style="color:var(--text);font-size:0.95rem;margin-top:1rem">'
                         'Why it matters</h4>')
            parts.append(f'<p>{_esc(why_text)}</p>')

        # Results table for this metric
        mk_vals = [(e, metric_data[mk].get(e)) for e in engines]
        mk_ranks = _compute_ranks(mk_vals, info["higher_better"])
        scored = [(e, v) for e, v in mk_vals if v is not None]
        scored.sort(key=lambda x: x[1], reverse=info["higher_better"])
        if scored:
            parts.append('<h4 style="color:var(--text);font-size:0.95rem;margin-top:0.75rem">'
                         'Results</h4>')
            parts.append('<table class="when-table" role="table">')
            parts.append(f'<thead><tr><th scope="col">Engine</th>'
                         f'<th scope="col">{_esc(info["short"])}</th>'
                         f'<th scope="col">Rank</th></tr></thead>')
            parts.append("<tbody>")
            for eng, val in scored:
                r = mk_ranks.get(eng, 99)
                parts.append(f'<tr><td>{_esc(_get_display_name(eng))}</td>'
                             f'<td>{_fmt(val)}</td>'
                             f'<td>#{r}</td></tr>')
            parts.append("</tbody></table>")

        if when_rows:
            parts.append('<h4 style="color:var(--text);font-size:0.95rem;margin-top:0.75rem">'
                         'When to prioritize</h4>')
            parts.append('<table class="when-table" role="table">')
            parts.append('<thead><tr><th scope="col">Use Case</th>'
                         '<th scope="col">Recommended Engine</th></tr></thead>')
            parts.append("<tbody>")
            for use_case, rec_engine in when_rows:
                parts.append(f'<tr><td>{_esc(use_case)}</td>'
                             f'<td>{_esc(rec_engine)}</td></tr>')
            parts.append("</tbody></table>")

        parts.append("</div>")

    parts.append("</section>")

    # ── Metric Explanation Cards ──────────────────────────────────────────────
    parts.append('<section aria-labelledby="hd-explained">')
    parts.append('<h2 id="hd-explained">Metrics Explained</h2>')
    parts.append('<div class="metric-cards">')
    for mk, info in METRIC_INFO.items():
        direction = "Higher is better" if info["higher_better"] else "Lower is better"
        dir_color = "var(--green)" if info["higher_better"] else "var(--amber)"
        parts.append('<div class="metric-card">')
        parts.append(f'  <h4>{_esc(info["name"])}</h4>')
        parts.append(f'  <p>{_esc(info["description"])}</p>')
        parts.append(f'  <p style="margin-top:0.5rem;color:{dir_color}">'
                     f'<strong>{direction}</strong></p>')
        parts.append("</div>")
    parts.append("</div></section>")

    # ── Per-document Details ──────────────────────────────────────────────────
    parts.append('<section aria-labelledby="hd-perdoc">')
    parts.append('<h2 id="hd-perdoc">Per-Document Scores</h2>')
    for eng in engines:
        d = results[eng]
        documents = d.get("documents", [])
        if not documents:
            continue
        parts.append(f'<details><summary>{_esc(_get_display_name(eng))} '
                     f'— {len(documents)} documents</summary>')
        parts.append('<div class="detail-content">')
        parts.append('<table class="detail-table" role="table">')
        parts.append('<thead><tr>'
                     '<th scope="col">Document</th>'
                     '<th scope="col">NID</th>'
                     '<th scope="col">TEDS</th>'
                     '<th scope="col">MHS</th>'
                     '<th scope="col">Overall</th>'
                     '</tr></thead>')
        parts.append("<tbody>")
        for doc in documents:
            doc_id = doc.get("document_id", "?")
            s = doc.get("scores", {})
            parts.append(f'<tr><td>{_esc(doc_id)}</td>'
                         f'<td>{_fmt(s.get("nid"))}</td>'
                         f'<td>{_fmt(s.get("teds"))}</td>'
                         f'<td>{_fmt(s.get("mhs"))}</td>'
                         f'<td>{_fmt(s.get("overall"))}</td></tr>')
        parts.append("</tbody></table></div></details>")
    parts.append("</section>")

    # ── Raw data ──────────────────────────────────────────────────────────────
    parts.append('<section aria-labelledby="hd-raw">')
    parts.append('<h2 id="hd-raw">Raw Data (JSON)</h2>')
    parts.append("<details><summary>Click to expand raw JSON data</summary>")
    parts.append('<div class="detail-content">'
                 '<pre style="overflow-x:auto;font-size:0.82rem;color:var(--text-dim)">')
    summary: Dict[str, Any] = {}
    for eng in engines:
        d = results[eng]
        summary[eng] = {
            "metrics": d.get("metrics", {}).get("score", {}),
            "table_detection": {
                k: d.get("table_detection", {}).get(k) for k in ["f1", "precision", "recall"]
            },
            "speed": {
                k: d.get("speed", {}).get(k) for k in ["elapsed_per_doc", "total_elapsed", "document_count"]
            },
        }
    parts.append(_esc(json.dumps(summary, indent=2, ensure_ascii=False)))
    parts.append("</pre></div></details></section>")

    # ── Close main + Footer ──────────────────────────────────────────────────
    parts.append("</main>")

    parts.append(f"""<footer role="contentinfo">
  <p>Generated by <strong>EdgeParse Benchmark Suite</strong> &middot; {run_date}</p>
  <p>Methodology: <a href="https://opendataloader.org/docs/benchmark" rel="noopener">opendataloader.org/docs/benchmark</a></p>
</footer>
</div>
</body>
</html>""")

    html_content = "\n".join(parts)
    output_path.write_text(html_content, encoding="utf-8")
    return output_path
