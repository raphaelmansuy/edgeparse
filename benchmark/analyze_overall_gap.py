#!/usr/bin/env python3
"""Find docs where small improvements would most impact Overall score."""
import sys, os
sys.path.insert(0, 'src')
from evaluator_reading_order import evaluate_reading_order
from evaluator_table import evaluate_table
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
    
    nid, _ = evaluate_reading_order(gt_md, pred_md)
    teds, _ = evaluate_table(gt_md, pred_md)
    mhs, _ = evaluate_heading_level(gt_md, pred_md)
    
    metrics = [v for v in [nid, teds, mhs] if v is not None]
    avg = sum(metrics) / len(metrics) if metrics else 0
    
    # Also compute docling scores
    dp = docling_dir / gt_path.name
    docling_avg = None
    if dp.exists():
        docling_md = dp.read_text()
        d_nid, _ = evaluate_reading_order(gt_md, docling_md)
        d_teds, _ = evaluate_table(gt_md, docling_md)
        d_mhs, _ = evaluate_heading_level(gt_md, docling_md)
        d_metrics = [v for v in [d_nid, d_teds, d_mhs] if v is not None]
        docling_avg = sum(d_metrics) / len(d_metrics) if d_metrics else 0
    
    gap = (docling_avg - avg) if docling_avg is not None else 0
    results.append((doc_id, avg, nid, teds, mhs, docling_avg, gap))

results.sort(key=lambda x: -x[6])  # Sort by gap (how much docling beats us)
print(f'Total docs: {len(results)}')
ep_overall = sum(r[1] for r in results) / len(results)
print(f'EP Overall: {ep_overall:.4f}')

print(f'\nTop 30 docs where Docling beats us most (gap to close):')
for doc_id, avg, nid, teds, mhs, davg, gap in results[:30]:
    nid_s = f'NID={nid:.3f}' if nid is not None else ''
    teds_s = f'TEDS={teds:.3f}' if teds is not None else ''
    mhs_s = f'MHS={mhs:.3f}' if mhs is not None else ''
    metrics_str = ' '.join(filter(None, [nid_s, teds_s, mhs_s]))
    davg_s = f'{davg:.3f}' if davg is not None else 'N/A'
    print(f'  {doc_id}: avg={avg:.3f} doc={davg_s} gap={gap:+.3f} | {metrics_str}')

# Find docs with middle-range NID (0.85-0.95) where small NID improvement would help
print(f'\nDocs with NID 0.80-0.95 (potential quick NID wins):')
mid_nid = [(d, a, n, t, m, da, g) for d, a, n, t, m, da, g in results if n is not None and 0.80 <= n <= 0.95]
mid_nid.sort(key=lambda x: x[2])
for doc_id, avg, nid, teds, mhs, davg, gap in mid_nid[:20]:
    davg_s = f'{davg:.3f}' if davg is not None else 'N/A'
    print(f'  {doc_id}: NID={nid:.3f} avg={avg:.3f} gap={gap:+.3f}')
