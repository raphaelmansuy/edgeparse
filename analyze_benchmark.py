#!/usr/bin/env python3
import json
from pathlib import Path

# Load evaluations
hybrid_eval = json.loads(Path('benchmark/prediction/edgeparse_hybrid/evaluation.json').read_text())
baseline_eval = json.loads(Path('benchmark/prediction/edgeparse/evaluation.json').read_text())

# Compare documents with tables
docs_with_tables = []
for doc_data in hybrid_eval['documents']:
    doc_id = doc_data['document_id']
    hybrid_teds = doc_data['scores'].get('teds')
    baseline_teds = None
    for baseline_doc in baseline_eval['documents']:
        if baseline_doc['document_id'] == doc_id:
            baseline_teds = baseline_doc['scores'].get('teds')
            break
    
    if hybrid_teds is not None:
        improvement = hybrid_teds - baseline_teds if baseline_teds is not None else 0
        docs_with_tables.append({
            'doc_id': doc_id,
            'hybrid_teds': hybrid_teds,
            'baseline_teds': baseline_teds,
            'improvement': improvement,
            'overall': doc_data['scores'].get('overall'),
        })

# Sort by improvement descending
docs_with_tables.sort(key=lambda x: x['improvement'], reverse=True)

print('Top 10 documents where hybrid helped:')
for i, d in enumerate(docs_with_tables[:10], 1):
    print(f"{i}. {d['doc_id']}: baseline={d['baseline_teds']:.4f} -> hybrid={d['hybrid_teds']:.4f} ({d['improvement']:+.4f})")

print('\nBottom 10 documents where hybrid hurt:')
for i, d in enumerate(docs_with_tables[-10:], 1):
    print(f"{i}. {d['doc_id']}: baseline={d['baseline_teds']:.4f} -> hybrid={d['hybrid_teds']:.4f} ({d['improvement']:+.4f})")

print(f'\nTotal docs with tables: {len(docs_with_tables)}')
print(f'Docs where hybrid improved: {sum(1 for d in docs_with_tables if d["improvement"] > 0.01)}')
print(f'Docs where hybrid hurt: {sum(1 for d in docs_with_tables if d["improvement"] < -0.01)}')
print(f'\nAverage improvement: {sum(d["improvement"] for d in docs_with_tables) / len(docs_with_tables):.4f}')
