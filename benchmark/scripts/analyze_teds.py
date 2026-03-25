#!/usr/bin/env python3
"""Analyze TEDS score distribution to find improvement opportunities."""
import json
import sys
from collections import Counter

with open('prediction/edgeparse/evaluation.json') as f:
    data = json.load(f)

# Get per-doc scores
docs = data.get('per_document', data.get('documents', []))
if isinstance(docs, dict):
    docs = list(docs.values())

# Analyze TEDS distribution
teds_scores = [(d.get('doc_id','?'), d.get('teds', d.get('TEDS',0))) for d in docs]
teds_scores.sort(key=lambda x: x[1])

# Group by score ranges
ranges = Counter()
for _,t in teds_scores:
    if t == 0.0:
        ranges['0.0'] += 1
    elif t < 0.3:
        ranges['0.0-0.3'] += 1
    elif t < 0.5:
        ranges['0.3-0.5'] += 1
    elif t < 0.7:
        ranges['0.5-0.7'] += 1
    elif t < 0.9:
        ranges['0.7-0.9'] += 1
    else:
        ranges['0.9-1.0'] += 1

print('TEDS score distribution:')
for k,v in sorted(ranges.items()):
    print(f'  {k}: {v} documents')

# Show medium-scoring docs
print()
mid_docs = [(doc,t) for doc,t in teds_scores if 0.1 < t < 0.5]
print(f'Docs with TEDS between 0.1 and 0.5 (improvable): {len(mid_docs)}')
if mid_docs:
    print(f'  Avg TEDS: {sum(t for _,t in mid_docs)/len(mid_docs):.3f}')

# Show docs with TEDS around 0.3-0.7
mid2_docs = [(doc,t) for doc,t in teds_scores if 0.3 <= t < 0.7]
print(f'\nDocs with TEDS 0.3-0.7 (quick wins): {len(mid2_docs)}')

all_teds = [t for _,t in teds_scores]
print(f'\nTotal docs: {len(all_teds)}')
print(f'Docs with TEDS=0.0: {sum(1 for t in all_teds if t==0.0)}')
print(f'Docs with TEDS=1.0: {sum(1 for t in all_teds if t==1.0)}')
print(f'Docs with TEDS>0.9: {sum(1 for t in all_teds if t>0.9)}')
print(f'Avg TEDS: {sum(all_teds)/len(all_teds):.4f}')

# Show the TEDS=0 docs
print('\nAll TEDS=0 docs:')
for doc,t in teds_scores:
    if t == 0.0:
        print(f'  {doc}: {t}')

# PBF analysis
pbf_scores = [(d.get('doc_id','?'), d.get('pbf', d.get('PBF',0))) for d in docs]
pbf_scores.sort(key=lambda x: x[1])
all_pbf = [t for _,t in pbf_scores]
print(f'\nPBF Distribution:')
pbf_ranges = Counter()
for _,p in pbf_scores:
    if p == 0.0:
        pbf_ranges['0.0'] += 1
    elif p < 0.3:
        pbf_ranges['0.0-0.3'] += 1
    elif p < 0.5:
        pbf_ranges['0.3-0.5'] += 1
    elif p < 0.7:
        pbf_ranges['0.5-0.7'] += 1
    elif p < 0.9:
        pbf_ranges['0.7-0.9'] += 1
    else:
        pbf_ranges['0.9-1.0'] += 1
for k,v in sorted(pbf_ranges.items()):
    print(f'  {k}: {v} documents')
print(f'Avg PBF: {sum(all_pbf)/len(all_pbf):.4f}')
print(f'Docs with PBF=0.0: {sum(1 for p in all_pbf if p==0.0)}')
print(f'Docs with PBF=1.0: {sum(1 for p in all_pbf if p==1.0)}')

# NID analysis
nid_scores = [(d.get('doc_id','?'), d.get('nid', d.get('NID',0))) for d in docs]
all_nid = [t for _,t in nid_scores]
print(f'\nNID avg: {sum(all_nid)/len(all_nid):.4f}')
print(f'Docs with NID<0.5: {sum(1 for n in all_nid if n<0.5)}')
print(f'Docs with NID>0.9: {sum(1 for n in all_nid if n>0.9)}')
