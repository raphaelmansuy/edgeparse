#!/usr/bin/env python3
"""Find worst-performing documents by PBF and TEDS metrics."""
import json

with open('/Users/raphaelmansuy/Github/03-working/edgeparse/benchmark/prediction/edgeparse/evaluation.json') as f:
    data = json.load(f)
docs = data['documents']

rows = []
for d in docs:
    s = d['scores']
    rows.append({
        'id': d['document_id'],
        'pbf': s.get('paragraph_boundary_f1'),
        'teds': s.get('teds'),
        'overall': s.get('overall', 0),
    })

rows_pbf = sorted([r for r in rows if r['pbf'] is not None], key=lambda x: x['pbf'])
print('=== Worst PBF (top 15) ===')
for r in rows_pbf[:15]:
    teds_str = f"{r['teds']:.3f}" if r['teds'] is not None else "  N/A"
    print(f"  {r['id']}  pbf={r['pbf']:.3f}  teds={teds_str}  overall={r['overall']:.3f}")

print()
rows_teds = sorted([r for r in rows if r['teds'] is not None], key=lambda x: x['teds'])
print('=== Worst TEDS (top 15, docs that have tables) ===')
for r in rows_teds[:15]:
    pbf_str = f"{r['pbf']:.3f}" if r['pbf'] is not None else "  N/A"
    print(f"  {r['id']}  teds={r['teds']:.3f}  pbf={pbf_str}  overall={r['overall']:.3f}")
