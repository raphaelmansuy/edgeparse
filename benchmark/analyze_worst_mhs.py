#!/usr/bin/env python3
"""Analyze the worst MHS docs: compare GT headings vs predicted headings."""

import json
import re
from pathlib import Path

benchmark_dir = Path(__file__).parent
gt_dir = benchmark_dir / "ground-truth" / "markdown"
pred_dir = benchmark_dir / "prediction" / "edgeparse" / "markdown"
eval_path = benchmark_dir / "prediction" / "edgeparse" / "evaluation.json"

with open(eval_path) as f:
    data = json.load(f)

# Get worst MHS docs
worst_docs = []
for doc in data["documents"]:
    s = doc["scores"]
    mhs = s.get("mhs")
    if mhs is not None and mhs < 0.6:
        worst_docs.append((doc["document_id"], mhs, s.get("nid", 0)))

worst_docs.sort(key=lambda x: x[1])

def extract_headings(md_text):
    """Extract markdown headings from text."""
    headings = []
    for line in md_text.split('\n'):
        m = re.match(r'^(#{1,6})\s+(.+)', line)
        if m:
            level = len(m.group(1))
            text = m.group(2).strip()
            headings.append((level, text))
    return headings

for did, mhs, nid in worst_docs:
    print(f"\n{'='*60}")
    print(f"Doc {did}: MHS={mhs:.4f}, NID={nid:.4f}")
    print(f"{'='*60}")
    
    gt_file = gt_dir / f"{did}.md"
    pred_file = pred_dir / f"{did}.md"
    
    gt_headings = []
    pred_headings = []
    
    if gt_file.exists():
        gt_headings = extract_headings(gt_file.read_text())
    else:
        print("  GT file not found!")
        
    if pred_file.exists():
        pred_headings = extract_headings(pred_file.read_text())
    else:
        print("  Pred file not found!")
    
    print(f"  GT headings ({len(gt_headings)}):")
    for level, text in gt_headings:
        print(f"    H{level}: {text[:80]}")
    
    print(f"  Pred headings ({len(pred_headings)}):")
    for level, text in pred_headings:
        print(f"    H{level}: {text[:80]}")
    
    # Show count diff
    gt_count = len(gt_headings)
    pred_count = len(pred_headings)
    if gt_count > pred_count:
        print(f"  -> UNDER-detected: missing {gt_count - pred_count} headings")
    elif pred_count > gt_count:
        print(f"  -> OVER-detected: {pred_count - gt_count} extra headings")
    else:
        print(f"  -> Same count but possibly wrong text/levels")
