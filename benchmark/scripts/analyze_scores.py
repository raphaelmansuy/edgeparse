#!/usr/bin/env python3
"""Analyze per-document scores to find improvement opportunities.
Uses the correct field names from evaluation.json.
"""
import json
from collections import Counter

with open('prediction/edgeparse/evaluation.json') as f:
    data = json.load(f)

docs = data['documents']
print(f"Total documents: {len(docs)}")

# Extract scores
all_teds = []
all_pbf = []
all_nid = []
zero_pbf_docs = []
zero_teds_docs = []
partial_teds_docs = []

for doc in docs:
    doc_id = doc['document_id']
    scores = doc['scores']
    
    tbf = scores.get('paragraph_boundary_f1', 0) or 0
    all_pbf.append((doc_id, tbf))
    if tbf == 0.0:
        zero_pbf_docs.append(doc_id)
    
    teds = scores.get('teds')
    if teds is not None:
        all_teds.append((doc_id, teds))
        if teds == 0.0:
            zero_teds_docs.append(doc_id)
        elif teds < 0.5:
            partial_teds_docs.append((doc_id, teds))
    
    nid = scores.get('nid', 0) or 0
    all_nid.append((doc_id, nid))

# TEDS distribution
print(f"\n=== TEDS Analysis ===")
print(f"Docs with tables in GT: {len(all_teds)} / {len(docs)}")
if all_teds:
    teds_vals = [t for _,t in all_teds]
    print(f"Average TEDS (only where GT has tables): {sum(teds_vals)/len(teds_vals):.4f}")
    print(f"Docs with TEDS=0.0: {len(zero_teds_docs)}")
    print(f"Docs with TEDS=1.0: {sum(1 for t in teds_vals if t==1.0)}")
    print(f"Docs with TEDS>0.8: {sum(1 for t in teds_vals if t>0.8)}")
    
    # Distribution
    ranges = Counter()
    for t in teds_vals:
        if t == 0.0: ranges['0.0'] += 1
        elif t < 0.3: ranges['0.01-0.3'] += 1
        elif t < 0.5: ranges['0.3-0.5'] += 1
        elif t < 0.7: ranges['0.5-0.7'] += 1
        elif t < 0.9: ranges['0.7-0.9'] += 1
        else: ranges['0.9-1.0'] += 1
    print("\nTEDS distribution:")
    for k,v in sorted(ranges.items()):
        print(f"  {k}: {v} docs")
    
    # Low TEDS docs (improvable)
    all_teds.sort(key=lambda x: x[1])
    print("\nBottom 20 TEDS docs:")
    for doc_id, t in all_teds[:20]:
        print(f"  {doc_id}: {t:.4f}")

# PBF distribution
print(f"\n=== PBF Analysis ===")
pbf_vals = [t for _,t in all_pbf]
print(f"Average PBF: {sum(pbf_vals)/len(pbf_vals):.4f}")
print(f"Docs with PBF=0.0: {len(zero_pbf_docs)}")
print(f"Docs with PBF=1.0: {sum(1 for p in pbf_vals if p==1.0)}")
print(f"Docs with PBF>0.8: {sum(1 for p in pbf_vals if p>0.8)}")

ranges = Counter()
for p in pbf_vals:
    if p == 0.0: ranges['0.0'] += 1
    elif p < 0.3: ranges['0.01-0.3'] += 1
    elif p < 0.5: ranges['0.3-0.5'] += 1
    elif p < 0.7: ranges['0.5-0.7'] += 1
    elif p < 0.9: ranges['0.7-0.9'] += 1
    else: ranges['0.9-1.0'] += 1
print("\nPBF distribution:")
for k,v in sorted(ranges.items()):
    print(f"  {k}: {v} docs")

# NID distribution
print(f"\n=== NID Analysis ===")
nid_vals = [t for _,t in all_nid]
print(f"Average NID: {sum(nid_vals)/len(nid_vals):.4f}")
print(f"Docs with NID<0.5: {sum(1 for n in nid_vals if n<0.5)}")
print(f"Docs with NID>0.9: {sum(1 for n in nid_vals if n>0.9)}")

ranges = Counter()
for n in nid_vals:
    if n < 0.5: ranges['<0.5'] += 1
    elif n < 0.7: ranges['0.5-0.7'] += 1
    elif n < 0.9: ranges['0.7-0.9'] += 1
    else: ranges['0.9-1.0'] += 1
print("\nNID distribution:")
for k,v in sorted(ranges.items()):
    print(f"  {k}: {v} docs")

# Summary metrics
print(f"\n=== Summary from evaluation.json ===")
if 'summary' in data:
    for k,v in data['summary'].items():
        print(f"  {k}: {v}")
elif 'metrics' in data:
    for k,v in data['metrics'].items():
        print(f"  {k}: {v}")
