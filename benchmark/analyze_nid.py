#!/usr/bin/env python3
"""Analyze worst NID docs to understand reading order problems."""

import json
from pathlib import Path
from rapidfuzz import fuzz

benchmark_dir = Path(__file__).parent
gt_dir = benchmark_dir / "ground-truth" / "markdown"
pred_dir = benchmark_dir / "prediction" / "edgeparse" / "markdown"
eval_path = benchmark_dir / "prediction" / "edgeparse" / "evaluation.json"

with open(eval_path) as f:
    data = json.load(f)

# Get worst NID docs
worst = []
for doc in data["documents"]:
    nid = doc["scores"].get("nid")
    if nid is not None and nid < 0.8:
        worst.append((doc["document_id"], nid))
worst.sort(key=lambda x: x[1])

for did, nid in worst[:15]:
    gt_file = gt_dir / f"{did}.md"
    pred_file = pred_dir / f"{did}.md"
    
    gt_text = gt_file.read_text() if gt_file.exists() else ""
    pred_text = pred_file.read_text() if pred_file.exists() else ""
    
    gt_len = len(gt_text)
    pred_len = len(pred_text)
    gt_words = len(gt_text.split())
    pred_words = len(pred_text.split())
    
    # Check text overlap
    gt_lines = set(l.strip() for l in gt_text.split('\n') if l.strip())
    pred_lines = set(l.strip() for l in pred_text.split('\n') if l.strip())
    common = gt_lines & pred_lines
    
    print(f"Doc {did}: NID={nid:.4f}")
    print(f"  GT: {gt_words} words, {gt_len} chars, {len(gt_lines)} lines")
    print(f"  Pred: {pred_words} words, {pred_len} chars, {len(pred_lines)} lines")
    print(f"  Common lines: {len(common)}/{len(gt_lines)} GT, {len(common)}/{len(pred_lines)} Pred")
    
    # Show first 100 chars of each
    print(f"  GT start: {gt_text[:100].replace(chr(10), '|')}")
    print(f"  Pred start: {pred_text[:100].replace(chr(10), '|')}")
    
    # Check if text is mostly same but reordered vs missing
    gt_words_set = set(gt_text.lower().split())
    pred_words_set = set(pred_text.lower().split())
    missing_words = gt_words_set - pred_words_set
    extra_words = pred_words_set - gt_words_set
    print(f"  Missing words: {len(missing_words)}, Extra words: {len(extra_words)}, Overlap: {len(gt_words_set & pred_words_set)}")
    print()
