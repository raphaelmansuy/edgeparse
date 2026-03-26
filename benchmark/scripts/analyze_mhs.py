#!/usr/bin/env python3
"""Analyze MHS and NID score distributions."""
import json
from collections import Counter

with open('prediction/edgeparse/evaluation.json') as f:
    d = json.load(f)
docs = d['documents']

# MHS analysis
mhs_scores = [(doc['document_id'], doc['scores'].get('mhs') or 0) for doc in docs]
mhs_scores.sort(key=lambda x: x[1])
print('MHS Distribution:')
ranges = Counter()
for _,v in mhs_scores:
    if v==0.0: ranges['0.0']+=1
    elif v<0.25: ranges['0.01-0.25']+=1
    elif v<0.5: ranges['0.25-0.5']+=1
    elif v<0.75: ranges['0.5-0.75']+=1
    elif v<1.0: ranges['0.75-1.0']+=1
    else: ranges['1.0']+=1
for k,v in sorted(ranges.items()):
    print(f'  {k}: {v} docs')
print(f'Avg MHS: {sum(v for _,v in mhs_scores)/len(mhs_scores):.4f}')
print(f'Docs with MHS=0: {sum(1 for _,v in mhs_scores if v==0.0)}')
print(f'Docs MHS=None: {sum(1 for d in docs if d["scores"].get("mhs") is None)}')
print()
print('Bottom 15 MHS docs:')
for doc_id,v in mhs_scores[:15]:
    print(f'  {doc_id}: {v:.4f}')

# NID analysis  
print()
nid_scores = [(doc['document_id'], doc['scores'].get('nid') or 0) for doc in docs]
nid_scores.sort(key=lambda x: x[1])
print('NID Distribution:')
ranges2 = Counter()
for _,v in nid_scores:
    if v < 0.5: ranges2['<0.5']+=1
    elif v < 0.7: ranges2['0.5-0.7']+=1
    elif v < 0.8: ranges2['0.7-0.8']+=1
    elif v < 0.9: ranges2['0.8-0.9']+=1
    else: ranges2['0.9-1.0']+=1
for k,v in sorted(ranges2.items()):
    print(f'  {k}: {v} docs')
print(f'Avg NID: {sum(v for _,v in nid_scores)/len(nid_scores):.4f}')
print()
print('Bottom 10 NID docs:')
for doc_id,v in nid_scores[:10]:
    print(f'  {doc_id}: {v:.4f}')

# TQS analysis
print()
tqs_scores = [(doc['document_id'], doc['scores'].get('text_quality_score') or 0) for doc in docs]
tqs_scores.sort(key=lambda x: x[1])
print(f'Avg TQS: {sum(v for _,v in tqs_scores)/len(tqs_scores):.4f}')
print('Bottom 10 TQS docs:')
for doc_id,v in tqs_scores[:10]:
    print(f'  {doc_id}: {v:.4f}')

# Overall per-doc  
print()
overall_scores = [(doc['document_id'], doc['scores'].get('overall') or 0) for doc in docs]
overall_scores.sort(key=lambda x: x[1])
print(f'Avg Overall: {sum(v for _,v in overall_scores)/len(overall_scores):.4f}')
print('Bottom 15 Overall docs:')
for doc_id,v in overall_scores[:15]:
    teds = docs[[d['document_id'] for d in docs].index(doc_id)]['scores'].get('teds')
    mhs = docs[[d['document_id'] for d in docs].index(doc_id)]['scores'].get('mhs')
    nid = docs[[d['document_id'] for d in docs].index(doc_id)]['scores'].get('nid')
    tqs = docs[[d['document_id'] for d in docs].index(doc_id)]['scores'].get('text_quality_score')
    print(f'  {doc_id}: overall={v:.3f} nid={nid:.3f} teds={teds} mhs={mhs:.3f} tqs={tqs:.3f}')
