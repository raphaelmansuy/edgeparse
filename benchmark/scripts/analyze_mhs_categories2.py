#!/usr/bin/env python3
"""Analyze all MHS=0 docs to identify patterns."""
import json
import os

# First find all MHS=0 docs
results_file = 'prediction/edgeparse/evaluation.json'
if not os.path.exists(results_file):
    print("No results file found")
    exit(1)

with open(results_file) as f:
    data = json.load(f)

documents = data.get('documents', [])

# Find all MHS=0 docs
mhs_zero_docs = []
for doc in documents:
    scores = doc.get('scores', {})
    mhs = scores.get('mhs', None)
    if mhs is not None and mhs == 0.0:
        mhs_zero_docs.append(doc['document_id'])

mhs_zero_docs.sort()
print(f"Total MHS=0 docs: {len(mhs_zero_docs)}")
print()

# For each, check: what does GT have as headings? 
# What does EdgeParse output?
gt_dir = 'ground-truth/markdown'
pred_dir = 'prediction/edgeparse/markdown'

categories = {
    'no_gt_headings': [],
    'pred_has_headings': [],  # EP has headings but wrong
    'pred_no_headings': [],   # EP has 0 headings, GT has headings
}

for doc_id in mhs_zero_docs:
    gt_path = f'{gt_dir}/{doc_id}.md'
    pred_path = f'{pred_dir}/{doc_id}.md'
    
    gt_headings = []
    pred_headings = []
    
    if os.path.exists(gt_path):
        with open(gt_path) as f:
            for line in f:
                if line.startswith('#'):
                    gt_headings.append(line.strip())
    
    if os.path.exists(pred_path):
        with open(pred_path) as f:
            for line in f:
                if line.startswith('#'):
                    pred_headings.append(line.strip())
    
    if not gt_headings:
        categories['no_gt_headings'].append(doc_id)
    elif pred_headings:
        categories['pred_has_headings'].append((doc_id, gt_headings, pred_headings))
    else:
        categories['pred_no_headings'].append((doc_id, gt_headings))

print(f"Category: no GT headings (shouldn't be MHS=0): {len(categories['no_gt_headings'])}")
if categories['no_gt_headings']:
    print(f"  {categories['no_gt_headings'][:10]}")

print(f"\nCategory: EP has headings but wrong (MHS=0 means 0% match): {len(categories['pred_has_headings'])}")
for doc_id, gt, pred in categories['pred_has_headings'][:5]:
    print(f"  {doc_id}")
    print(f"    GT:   {gt[:2]}")
    print(f"    Pred: {pred[:2]}")

print(f"\nCategory: EP has 0 headings, GT has headings: {len(categories['pred_no_headings'])}")
print()
for doc_id, gt in categories['pred_no_headings']:
    print(f"  {doc_id}: GT={gt[:2]}")

print(f"\nTotal: {len(categories['no_gt_headings']) + len(categories['pred_has_headings']) + len(categories['pred_no_headings'])}")
