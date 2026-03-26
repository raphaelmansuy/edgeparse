#!/usr/bin/env python3
"""Analyze GT headings for MHS=0 docs to understand heading failure patterns."""
import json
import re
from pathlib import Path

with open('prediction/edgeparse/evaluation.json') as f:
    d = json.load(f)
docs = d['documents']

# Find MHS=0 docs
mhs_zero_docs = [
    doc['document_id'] 
    for doc in docs 
    if (doc['scores'].get('mhs') or 0) == 0.0 and doc['scores'].get('mhs') is not None
]

print(f"MHS=0 docs: {len(mhs_zero_docs)}")

gt_dir = Path('ground-truth/markdown')
pred_dir = Path('prediction/edgeparse')

heading_pattern = re.compile(r'^(#{1,6})\s+(.+)$', re.MULTILINE)
figure_pattern = re.compile(r'^(?:figure|fig\.?)\s+\d', re.IGNORECASE)

figure_heading_docs = 0
no_heading_gt_docs = 0
other_docs = 0

for doc_id in mhs_zero_docs[:50]:  # analyze first 50
    gt_file = gt_dir / f"{doc_id}.md"
    pred_file = pred_dir / f"{doc_id}.md"
    
    if not gt_file.exists():
        continue
    
    gt_text = gt_file.read_text()
    pred_text = pred_file.read_text() if pred_file.exists() else ''
    
    gt_headings = heading_pattern.findall(gt_text)
    pred_headings = heading_pattern.findall(pred_text)
    
    if not gt_headings:
        no_heading_gt_docs += 1
        continue
    
    # Check if GT headings are figure captions
    fig_headings = [h for level, h in gt_headings if figure_pattern.match(h)]
    non_fig_headings = [h for level, h in gt_headings if not figure_pattern.match(h)]
    
    is_figure_page = len(fig_headings) > 0 and len(non_fig_headings) == 0
    is_mixed = len(fig_headings) > 0 and len(non_fig_headings) > 0
    
    if is_figure_page:
        figure_heading_docs += 1
        
    pred_heading_texts = [h for level, h in pred_headings]
    
    print(f"\n{doc_id} (MHS=0):")
    print(f"  GT headings ({len(gt_headings)}):")
    for level, text in gt_headings[:3]:
        print(f"    {level} '{text[:70]}'")
    print(f"  EdgeParse headings ({len(pred_headings)}):")
    for level, text in pred_headings[:3]:
        print(f"    {level} '{text[:70]}'")
    print(f"  Is figure page (only fig headings): {is_figure_page}")

print(f"\n=== Summary ===")
print(f"MHS=0 docs analyzed: {min(50, len(mhs_zero_docs))}")
print(f"  Docs where GT only has figure headings: {figure_heading_docs}")
print(f"  Docs where GT has no headings: {no_heading_gt_docs}")
