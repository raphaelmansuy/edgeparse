#!/usr/bin/env python3
"""Show font sizes for a doc's elements."""
import json, sys

doc_id = sys.argv[1] if len(sys.argv) > 1 else "01030000000170"
path = f"/tmp/edgeparse_debug/{doc_id}.json"

with open(path) as f:
    data = json.load(f)

elements = data.get('elements', data.get('kids', []))
print(f'Total elements: {len(elements)}')

for i, e in enumerate(elements):
    tp = e.get('type', '?')
    fs = e.get('font size', '?')
    ct = e.get('content', '')[:60]
    print(f'  [{i:2d}] {tp:12s} fs={fs}: {ct!r}')
