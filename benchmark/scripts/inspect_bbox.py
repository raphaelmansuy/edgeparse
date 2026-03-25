#!/usr/bin/env python3
"""Show all nodes with bounding boxes from extracted JSON."""
import json
import sys
import os
from pathlib import Path

def analyze_doc(json_path, gt_path=None):
    doc_id = Path(json_path).stem
    print(f"\n=== {doc_id} ===")
    
    if gt_path and os.path.exists(gt_path):
        with open(gt_path) as f:
            lines = f.readlines()
        headings = [l.strip() for l in lines if l.startswith('#')]
        print(f"GT headings: {headings[:5]}")
    
    with open(json_path) as f:
        d = json.load(f)
    
    kids = d.get('kids', [])
    print(f"Total elements: {len(kids)}")
    print(f"{'#':>3} {'type':12s} {'fs':>6} {'y0':>8} {'y1':>8} {'wid':>8} content")
    print("-" * 100)
    for i, node in enumerate(kids[:25]):
        t = node.get('type', '?')
        font = node.get('font', '?')
        size = node.get('font size', '?')
        content = node.get('content', '')[:50]
        bb = node.get('bounding box', [0, 0, 0, 0])
        if len(bb) >= 4:
            x0, y0, x1, y1 = bb[:4]
        else:
            x0 = y0 = x1 = y1 = 0
        width = x1 - x0
        print(f"{i:>3} {t:12s} {str(size):>6} {y0:>8.1f} {y1:>8.1f} {width:>8.1f} {content!r}")

docs = [
    ('/tmp/outbatch/01030000000009.json', 'benchmark/ground-truth/markdown/01030000000009.md'),
    ('/tmp/outbatch/01030000000010.json', 'benchmark/ground-truth/markdown/01030000000010.md'),
    ('/tmp/outbatch/01030000000017.json', 'benchmark/ground-truth/markdown/01030000000017.md'),
    ('/tmp/outbatch/01030000000030.json', 'benchmark/ground-truth/markdown/01030000000030.md'),
    ('/tmp/out20/01030000000020.json', 'benchmark/ground-truth/markdown/01030000000020.md'),
]

for json_path, gt_path in docs:
    if os.path.exists(json_path):
        analyze_doc(json_path, gt_path)
