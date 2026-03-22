#!/usr/bin/env python3
"""Analyze false positive heading patterns."""
import re
from pathlib import Path

gt_dir = Path(__file__).parent / 'ground-truth' / 'markdown'
pred_dir = Path(__file__).parent / 'prediction' / 'edgeparse' / 'markdown'

heading_re = re.compile(r'^#{1,6}\s+(.*)$', re.MULTILINE)

all_fp = []
for gt in sorted(gt_dir.glob('*.md')):
    pred = pred_dir / gt.name
    if not pred.exists():
        continue
    gt_h_set = set(h.strip().lower() for h in heading_re.findall(gt.read_text()))
    pred_h = heading_re.findall(pred.read_text())
    for h in pred_h:
        if h.strip().lower() not in gt_h_set:
            all_fp.append((gt.stem, h.strip()))

print(f'Total false positive headings: {len(all_fp)}')
print()

# Math symbols
math_chars = set('\u2202\u0393\u226a\u226b\u2200\u2203\u2211\u220f\u222b\u2264\u2265\u2260\u2248\u2245\u2282\u2283\u2208\u2209\u2205\u221e\u00bc\u00bd\u00be\u00b1\u00d7\u00f7\u00fe\u221a')
print('MATH pattern FPs:')
for stem, h in all_fp:
    if any(c in math_chars for c in h):
        print(f'  {stem}: {h[:80]}')

print()
print('COMMA+PERIOD FPs:')
for stem, h in all_fp:
    if h.endswith('.') and ',' in h:
        print(f'  {stem}: {h[:80]}')

print()
print('All FPs:')
for stem, h in all_fp:
    print(f'  {stem}: {h[:80]}')
