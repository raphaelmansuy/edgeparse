#!/usr/bin/env python3
"""Find lowest-scoring docs across all metrics."""
import json, os

bench = os.path.join(os.path.dirname(__file__), '..', 'benchmark')
ev = json.load(open(os.path.join(bench, 'prediction/edgeparse/evaluation.json')))
docs = ev['documents']

def fmt(v):
    return f'{v:.3f}' if v is not None else 'N/A'

doc_overall = []
for d in docs:
    s = d['scores']
    doc_overall.append((
        s.get('overall', 0), d['document_id'],
        s.get('nid'), s.get('teds'), s.get('mhs'), s.get('text_quality_score')
    ))
doc_overall.sort(key=lambda x: x[0])

print('20 lowest Overall docs:')
for ov, did, nid, teds, mhs, tqs in doc_overall[:20]:
    print(f'  {did}: overall={fmt(ov)} nid={fmt(nid)} teds={fmt(teds)} mhs={fmt(mhs)} tqs={fmt(tqs)}')

print()
print('20 lowest NID docs:')
nid_docs = [(d['scores'].get('nid',1), d['document_id']) for d in docs if d['scores'].get('nid') is not None]
nid_docs.sort(key=lambda x: x[0])
for nid, did in nid_docs[:20]:
    tqs = next(d['scores'].get('text_quality_score') for d in docs if d['document_id'] == did)
    print(f'  {did}: nid={fmt(nid)} tqs={fmt(tqs)}')
