#!/usr/bin/env python3
"""Compute per-doc MHS scores and find worst docs."""
import sys, os
sys.path.insert(0, 'src')
from evaluator_heading_level import evaluate_heading_level
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
    mhs, mhs_s = evaluate_heading_level(gt_md, pred_md)
    if mhs is None:
        continue

    docling_mhs = None
    dp = docling_dir / gt_path.name
    if dp.exists():
        docling_mhs, _ = evaluate_heading_level(gt_md, dp.read_text())

    results.append((doc_id, mhs, mhs_s, docling_mhs))

results.sort(key=lambda x: x[1])
print(f'Total MHS docs: {len(results)}')
print(f'\nWorst 30 MHS docs:')
for doc_id, mhs, mhs_s, dmhs in results[:30]:
    ds = f'{dmhs:.3f}' if dmhs is not None else 'N/A'
    gap = f'{(dmhs-mhs):+.3f}' if dmhs is not None else ''
    print(f'  {doc_id}: EP={mhs:.3f} (S={mhs_s:.3f}) DOC={ds} {gap}')

print(f'\nDocs where docling beats us by >=0.2 on MHS:')
big = [(d, m, ms, dm) for d, m, ms, dm in results if dm is not None and dm - m >= 0.2]
big.sort(key=lambda x: x[3]-x[1], reverse=True)
for doc_id, mhs, mhs_s, dmhs in big:
    print(f'  {doc_id}: EP={mhs:.3f} DOC={dmhs:.3f} gap={dmhs-mhs:+.3f}')

# Count pred headings vs gt headings per worst doc
print(f'\nHeading counts for worst 15 docs:')
for doc_id, mhs, mhs_s, dmhs in results[:15]:
    gt_md = (gt_dir / f'{doc_id}.md').read_text()
    pred_md = (pred_dir / f'{doc_id}.md').read_text()
    gt_h = [l for l in gt_md.split('\n') if l.startswith('#')]
    pred_h = [l for l in pred_md.split('\n') if l.startswith('#')]
    print(f'  {doc_id}: GT={len(gt_h)}h PRED={len(pred_h)}h MHS={mhs:.3f}')
    for h in gt_h[:3]:
        print(f'    GT: {h[:70]}')
    for h in pred_h[:3]:
        print(f'    PR: {h[:70]}')
