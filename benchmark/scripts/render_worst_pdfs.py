#!/usr/bin/env python3
"""Render worst-performing PDFs as PNG tiles with side-by-side GT vs EdgeParse text.

Outputs to benchmark/ground-truth/png/<doc_id>/ :
  - page_01.png, page_02.png ...   raw PDF page renders (150 dpi)
  - diff.txt                       ground-truth vs EdgeParse text comparison
  - summary.txt                    scores + key diff stats

Usage:
  python3 scripts/render_worst_pdfs.py [--n 15] [--metric pbf|teds|both]
"""
from __future__ import annotations

import argparse
import json
import shutil
import textwrap
from pathlib import Path

import fitz  # PyMuPDF


BASE = Path(__file__).parent.parent
EVAL_JSON   = BASE / "prediction/edgeparse/evaluation.json"
PDF_DIR     = BASE / "pdfs"
GT_MD_DIR   = BASE / "ground-truth/markdown"
PRED_MD_DIR = BASE / "prediction/edgeparse/markdown"
PNG_DIR     = BASE / "ground-truth/png"

DPI = 150
MAX_PAGES = 4  # render at most first N pages per PDF


def load_worst(n: int, metric: str) -> list[dict]:
    data = json.loads(EVAL_JSON.read_text())
    docs = data["documents"]
    rows = []
    for d in docs:
        s = d["scores"]
        rows.append({
            "id": d["document_id"],
            "pbf":  s.get("paragraph_boundary_f1"),
            "teds": s.get("teds"),
            "nid":  s.get("nid"),
            "overall": s.get("overall", 0),
        })

    if metric == "pbf":
        rows = [r for r in rows if r["pbf"] is not None]
        rows.sort(key=lambda x: x["pbf"])
    elif metric == "teds":
        rows = [r for r in rows if r["teds"] is not None]
        rows.sort(key=lambda x: x["teds"])
    else:  # both — union of worst per metric
        pbf_worst = sorted([r for r in rows if r["pbf"]  is not None], key=lambda x: x["pbf"])[:n]
        teds_worst = sorted([r for r in rows if r["teds"] is not None], key=lambda x: x["teds"])[:n]
        seen, combined = set(), []
        for r in pbf_worst + teds_worst:
            if r["id"] not in seen:
                seen.add(r["id"])
                combined.append(r)
        return combined

    return rows[:n]


def render_pdf_pages(pdf_path: Path, out_dir: Path) -> int:
    """Render first MAX_PAGES pages as PNG files. Returns actual page count rendered."""
    doc = fitz.open(str(pdf_path))
    count = min(len(doc), MAX_PAGES)
    mat = fitz.Matrix(DPI / 72, DPI / 72)
    for i in range(count):
        page = doc[i]
        pix = page.get_pixmap(matrix=mat, alpha=False)
        out_path = out_dir / f"page_{i+1:02d}.png"
        pix.save(str(out_path))
    doc.close()
    return count


def write_diff(doc_id: str, out_dir: Path, scores: dict) -> None:
    """Write ground-truth vs EdgeParse text side-by-side diff."""
    gt_path   = GT_MD_DIR   / f"{doc_id}.md"
    pred_path = PRED_MD_DIR / f"{doc_id}.md"

    gt_text   = gt_path.read_text(errors="replace")   if gt_path.exists()   else "(no ground-truth)"
    pred_text = pred_path.read_text(errors="replace") if pred_path.exists() else "(no prediction)"

    # Build readable summary
    with open(out_dir / "diff.txt", "w") as f:
        f.write(f"=== GROUND TRUTH ({doc_id}) ===\n\n")
        f.write(gt_text[:6000])
        if len(gt_text) > 6000:
            f.write(f"\n... [{len(gt_text) - 6000} more chars] ...\n")
        f.write("\n\n")
        f.write(f"=== EDGEPARSE OUTPUT ({doc_id}) ===\n\n")
        f.write(pred_text[:6000])
        if len(pred_text) > 6000:
            f.write(f"\n... [{len(pred_text) - 6000} more chars] ...\n")

    # Paragraph count comparison
    gt_paras   = [p.strip() for p in gt_text.split("\n\n")   if p.strip()]
    pred_paras = [p.strip() for p in pred_text.split("\n\n") if p.strip()]

    with open(out_dir / "summary.txt", "w") as f:
        f.write(f"Document: {doc_id}\n")
        f.write(f"Scores:\n")
        for k, v in scores.items():
            if isinstance(v, float):
                f.write(f"  {k:30s} = {v:.4f}\n")
            elif v is not None:
                f.write(f"  {k:30s} = {v}\n")
            else:
                f.write(f"  {k:30s} = N/A\n")
        f.write(f"\nGround-truth paragraphs : {len(gt_paras)}\n")
        f.write(f"EdgeParse paragraphs    : {len(pred_paras)}\n")
        f.write(f"GT word count           : {len(gt_text.split())}\n")
        f.write(f"EdgeParse word count    : {len(pred_text.split())}\n")
        f.write(f"\nGT file    : {gt_path}\n")
        f.write(f"Pred file  : {pred_path}\n")
        f.write(f"PDF        : {PDF_DIR / (doc_id + '.pdf')}\n")

    # Write structure diff — show where paragraphs diverge
    with open(out_dir / "para_diff.txt", "w") as f:
        f.write(f"=== PARAGRAPH STRUCTURE DIFF ({doc_id}) ===\n\n")
        max_p = max(len(gt_paras), len(pred_paras))
        for i in range(min(max_p, 60)):
            gt_p   = gt_paras[i].replace("\n", " ")[:120]   if i < len(gt_paras)   else "(MISSING)"
            pred_p = pred_paras[i].replace("\n", " ")[:120] if i < len(pred_paras) else "(MISSING)"
            match = "OK " if gt_p == pred_p else "!!!"
            f.write(f"[{i+1:03d}] {match}\n")
            f.write(f"  GT  : {gt_p}\n")
            f.write(f"  EP  : {pred_p}\n")
            f.write("\n")


def process_doc(row: dict) -> None:
    doc_id = row["id"]
    out_dir = PNG_DIR / doc_id
    out_dir.mkdir(parents=True, exist_ok=True)

    pdf_path = PDF_DIR / f"{doc_id}.pdf"
    if not pdf_path.exists():
        print(f"  [SKIP] No PDF: {pdf_path}")
        return

    print(f"  Rendering {doc_id} (pbf={row['pbf']}, teds={row['teds']})...")
    n = render_pdf_pages(pdf_path, out_dir)
    write_diff(doc_id, out_dir, row)
    print(f"    → {n} pages → {out_dir}")


def main():
    parser = argparse.ArgumentParser(description="Render worst-performing PDFs as PNGs.")
    parser.add_argument("--n", type=int, default=20, help="Number of worst docs per metric")
    parser.add_argument("--metric", choices=["pbf", "teds", "both"], default="both")
    args = parser.parse_args()

    worst = load_worst(args.n, args.metric)
    print(f"Processing {len(worst)} worst documents (metric={args.metric})...")
    PNG_DIR.mkdir(parents=True, exist_ok=True)

    for row in worst:
        process_doc(row)

    print(f"\nDone. PNG output → {PNG_DIR}")
    print(f"  {len(list(PNG_DIR.iterdir()))} document directories created.")


if __name__ == "__main__":
    main()
