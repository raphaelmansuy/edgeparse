#!/usr/bin/env python3
"""Analyze heading count mismatches between GT and prediction."""
import sys, re
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / 'src'))

gt_dir = Path(__file__).parent / 'ground-truth' / 'markdown'
pred_dir = Path(__file__).parent / 'prediction' / 'edgeparse' / 'markdown'

heading_re = re.compile(r'^#{1,6}\s+(.*)$', re.MULTILINE)

results = []
for gt in sorted(gt_dir.glob('*.md')):
    pred = pred_dir / gt.name
    if not pred.exists():
        continue
    gt_h = heading_re.findall(gt.read_text())
    pred_h = heading_re.findall(pred.read_text())
    fp = max(0, len(pred_h) - len(gt_h))
    fn = max(0, len(gt_h) - len(pred_h))
    results.append((gt.stem, len(gt_h), len(pred_h), fp, fn))

# Sort by false positive excess
results.sort(key=lambda x: x[3], reverse=True)
print('Top 15 false-positive-heavy docs (pred > gt):')
for stem, gt_c, pred_c, fp, fn in results[:15]:
    print(f'  {stem}: gt={gt_c} pred={pred_c} excess={fp}')

print()
print('Top 15 false-negative-heavy docs (gt > pred):')
results.sort(key=lambda x: x[4], reverse=True)
for stem, gt_c, pred_c, fp, fn in results[:15]:
    print(f'  {stem}: gt={gt_c} pred={pred_c} missing={fn}')

# Also show the actual false positive headings for top FP docs
print()
print('=== False positive heading text examples ===')
results.sort(key=lambda x: x[3], reverse=True)
for stem, gt_c, pred_c, fp, fn in results[:10]:
    if fp == 0:
        break
    pred = pred_dir / f'{stem}.md'
    gt = gt_dir / f'{stem}.md'
    pred_h = heading_re.findall(pred.read_text())
    gt_h_set = set(h.strip().lower() for h in heading_re.findall(gt.read_text()))
    print(f'\n  {stem} (gt={gt_c}, pred={pred_c}):')
    for h in pred_h:
        marker = '  FP' if h.strip().lower() not in gt_h_set else '  ok'
        print(f'    {marker}: {h[:70]}')
