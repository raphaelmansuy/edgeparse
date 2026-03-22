#!/usr/bin/env python3
"""Analyze element layout for a given doc."""
import json, sys

doc_id = sys.argv[1] if len(sys.argv) > 1 else "01030000000031"
path = f"/tmp/edgeparse_debug/{doc_id}.json"

with open(path) as f:
    data = json.load(f)

elements = data.get('elements', data.get('kids', []))
print(f'Total elements: {len(elements)}')

for i, e in enumerate(elements[:30]):
    etype = e.get('type', '?')
    text = e.get('text_content', e.get('value', ''))[:100]
    bbox = e.get('bbox', {})
    x = bbox.get('left_x', 0)
    y = bbox.get('top_y', 0)
    rx = bbox.get('right_x', 0)
    w = rx - x
    print(f'  [{i:2d}] {etype:12s} x={x:6.1f} rx={rx:6.1f} w={w:5.0f} y={y:6.1f}: {text!r}')
