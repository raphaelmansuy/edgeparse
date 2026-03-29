#!/usr/bin/env python3
import json
all_docs = json.loads(open('benchmark/prediction/edgeparse_hybrid/evaluation.json').read())['documents']
worst = sorted(all_docs, key=lambda x: x['scores'].get('overall', 0))[:10]
print('Worst performing documents (hybrid mode):')
for d in worst:
    print(f"{d['document_id']}: overall={d['scores']['overall']:.4f}, teds={d['scores'].get('teds', 'N/A')}, mhs={d['scores'].get('mhs', 'N/A')}")

print('\n\nDocuments with low TEDS scores (table issues):')
teds_docs = [(d['document_id'], d['scores'].get('teds', None)) for d in all_docs if d['scores'].get('teds') is not None and d['scores'].get('teds', 1) < 0.3]
teds_docs.sort(key=lambda x: x[1])
for doc_id, teds in teds_docs[:10]:
    print(f"{doc_id}: TEDS={teds:.4f}")
