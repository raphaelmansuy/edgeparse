#!/usr/bin/env python3
"""Analyze per-doc TEDS scores to find improvement targets."""
import sys, os
sys.path.insert(0, 'src')
from evaluator_table import evaluate_table
from pathlib import Path

gt_dir = Path('ground-truth/markdown')
pred_dir = Path('prediction/edgeparse/markdown')
docling_dir = Path('prediction/docling/markdown')

results = []
for gt_path in sorted(gt_dir.glob('*.md')):
    doc_id = gt_path.stem
    pred_path = pred_dir / gt_path.name
    if not pred_path.exists():
        continue
    gt_md = gt_path.read_text()
    pred_md = pred_path.read_text()
    teds, teds_s = evaluate_table(gt_md, pred_md)
    if teds is None:
        continue

    docling_teds = None
    dp = docling_dir / gt_path.name
    if dp.exists():
        docling_teds, _ = evaluate_table(gt_md, dp.read_text())

    results.append((doc_id, teds, teds_s, docling_teds))

results.sort(key=lambda x: x[1])
print(f'Total TEDS docs: {len(results)}')
avg_ep = sum(t for _, t, _, _ in results) / len(results)
avg_doc = sum(dt for _, _, _, dt in results if dt is not None) / sum(1 for _, _, _, dt in results if dt is not None)
print(f'Average EP TEDS: {avg_ep:.4f}')
print(f'Average DOC TEDS: {avg_doc:.4f}')

print(f'\nAll TEDS docs sorted by score:')
for doc_id, teds, teds_s, dteds in results:
    ds = f'{dteds:.3f}' if dteds is not None else 'N/A'
    gap = f'{(dteds-teds):+.3f}' if dteds is not None else ''
    struct_flag = '*' if teds_s > teds + 0.1 else ' '
    print(f'  {doc_id}: EP={teds:.3f} ST={teds_s:.3f}{struct_flag} DOC={ds} {gap}')

print(f'\nDocs where structure is good (TEDS-S > 0.8) but content is bad (TEDS < 0.7):')
struct_issues = [(d, t, ts, dt) for d, t, ts, dt in results if ts > 0.8 and t < 0.7]
for doc_id, teds, teds_s, dteds in struct_issues:
    print(f'  {doc_id}: TEDS={teds:.3f} TEDS-S={teds_s:.3f}')

print(f'\nDocs where structure is bad (TEDS-S < 0.5):')
bad_struct = [(d, t, ts, dt) for d, t, ts, dt in results if ts < 0.5]
for doc_id, teds, teds_s, dteds in bad_struct:
    ds = f'{dteds:.3f}' if dteds is not None else 'N/A'
    print(f'  {doc_id}: TEDS={teds:.3f} TEDS-S={teds_s:.3f} DOC={ds}')
