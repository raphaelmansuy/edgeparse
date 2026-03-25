#!/usr/bin/env python3
"""Inspect JSON for more MHS=0 docs."""
import json
import os
from pathlib import Path

def analyze_doc(json_path, gt_path=None):
    doc_id = Path(json_path).stem
    print(f"\n=== {doc_id} ===")
    
    if gt_path and os.path.exists(gt_path):
        with open(gt_path) as f:
            lines = f.readlines()
        headings = [l.strip() for l in lines if l.startswith('#')]
        print(f"GT headings: {headings[:3]}")
    
    with open(json_path) as f:
        d = json.load(f)
    
    kids = d.get('kids', [])
    print(f"Total elements: {len(kids)}")
    print(f"{'#':>3} {'type':10s} {'fs':>7} {'w':>7} {'y0':>8} {'y1':>8} content")
    print("-" * 90)
    for i, node in enumerate(kids[:20]):
        t = node.get('type', '?')
        size = node.get('font size', '?')
        content = node.get('content', '')[:50]
        bb = node.get('bounding box', [0, 0, 0, 0])
        if len(bb) >= 4:
            x0, y0, x1, y1 = bb[:4]
        else:
            x0 = y0 = x1 = y1 = 0
        width = x1 - x0
        print(f"{i:>3} {t:10s} {str(size):>7} {width:>7.1f} {y0:>8.1f} {y1:>8.1f} {content!r}")

base_json = '/tmp/outmore'
base_gt = '/Users/raphaelmansuy/Github/03-working/edgeparse/benchmark/ground-truth/markdown'
docs = ['01030000000064', '01030000000074', '01030000000093', '01030000000120', '01030000000129']

for doc_id in docs:
    json_path = f'{base_json}/{doc_id}.json'
    gt_path = f'{base_gt}/{doc_id}.md'
    if os.path.exists(json_path):
        analyze_doc(json_path, gt_path)
