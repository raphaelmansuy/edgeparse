#!/usr/bin/env python3
"""Show all nodes and their font sizes from extracted JSON to compare with GT headings."""
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
    for i, node in enumerate(kids[:20]):
        t = node.get('type', '?')
        font = node.get('font', '?')
        size = node.get('font size', '?')
        content = node.get('content', '')[:60]
        bb = node.get('bounding box', [])
        print(f"  [{i}] type={t:12s} font_size={size:5} font={font[:20]:20s} content={content!r}")

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
