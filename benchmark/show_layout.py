#!/usr/bin/env python3
"""Show element layout with bboxes for a doc."""
import json, sys

doc_id = sys.argv[1] if len(sys.argv) > 1 else "01030000000031"
path = f"/tmp/edgeparse_debug/{doc_id}.json"

with open(path) as f:
    data = json.load(f)

elements = data.get('elements', data.get('kids', []))
print(f'Total elements: {len(elements)}')

for i, e in enumerate(elements):
    bb = e.get('bounding box', [0,0,0,0])
    ct = e.get('content', '')[:80]
    tp = e.get('type', '?')
    pg = e.get('page number', '?')
    print(f'[{i:2d}] pg{pg} {tp:12s} x={bb[0]:6.1f}-{bb[2]:6.1f} y={bb[1]:6.1f}-{bb[3]:6.1f}: {ct!r}')
