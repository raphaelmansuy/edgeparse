#!/usr/bin/env python3
import sys, statistics
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / 'src'))
from evaluator import _evaluate_single_document as evaluate_document

gt_dir = Path(__file__).parent / 'ground-truth' / 'markdown'
pred_dir = Path(__file__).parent / 'prediction' / 'edgeparse' / 'markdown'

results = []
for gt in sorted(gt_dir.glob('*.md')):
    doc_id = gt.stem
    pred = pred_dir / gt.name
    if pred.exists():
        scores = evaluate_document(doc_id, gt, pred)
        results.append(scores)

# Sort by overall
results.sort(key=lambda x: x.overall if x.overall is not None else 1)
print('Worst 20 overall:')
for s in results[:20]:
    nid = f'{s.nid:.3f}' if s.nid is not None else 'N/A'
    teds = f'{s.teds:.3f}' if s.teds is not None else 'N/A'
    mhs = f'{s.mhs:.3f}' if s.mhs is not None else 'N/A'
    print(f'  {s.document_id}: overall={s.overall:.3f} nid={nid} teds={teds} mhs={mhs}')

# Means
nids = [s.nid for s in results if s.nid is not None]
tedss = [s.teds for s in results if s.teds is not None]
mhss = [s.mhs for s in results if s.mhs is not None]
overalls = [s.overall for s in results if s.overall is not None]
print(f'\nNID={statistics.mean(nids):.4f}(n={len(nids)}) TEDS={statistics.mean(tedss):.4f}(n={len(tedss)}) MHS={statistics.mean(mhss):.4f}(n={len(mhss)})')
print(f'Overall={statistics.mean(overalls):.4f}(n={len(overalls)})')

# Worst TEDS
teds_results = sorted([s for s in results if s.teds is not None], key=lambda x: x.teds)
print('\nWorst 10 TEDS:')
for s in teds_results[:10]:
    print(f'  {s.document_id}: teds={s.teds:.3f}')

# Worst MHS
mhs_results = sorted([s for s in results if s.mhs is not None], key=lambda x: x.mhs)
print('\nWorst 15 MHS:')
for s in mhs_results[:15]:
    print(f'  {s.document_id}: mhs={s.mhs:.3f}')

# Worst NID
nid_results = sorted(results, key=lambda x: x.nid if x.nid is not None else 1)
print('\nWorst 10 NID:')
for s in nid_results[:10]:
    print(f'  {s.document_id}: nid={s.nid:.3f}')
