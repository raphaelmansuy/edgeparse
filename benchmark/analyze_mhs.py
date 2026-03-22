"""Analyze MHS (heading hierarchy) scores per doc to find improvement targets."""
import os
import json
import sys

# Find the most recent benchmark results
reports_dir = 'reports'
jsons = sorted([f for f in os.listdir(reports_dir) if f.endswith('.json')], reverse=True)
if not jsons:
    print("No benchmark JSON found")
    sys.exit(1)

latest = jsons[0]
print(f"Using: {latest}")
with open(os.path.join(reports_dir, latest)) as f:
    data = json.load(f)

# Find edgeparse results
ep = None
for engine in data.get('engines', []):
    if engine.get('engine') == 'edgeparse':
        ep = engine
        break

if not ep:
    print("No edgeparse results found")
    sys.exit(1)

# Get per-doc MHS scores
mhs_scores = []
for doc in ep.get('documents', []):
    doc_id = doc.get('document_id', '')
    metrics = doc.get('metrics', {})
    mhs = metrics.get('mhs')
    if mhs is not None:
        mhs_scores.append((doc_id, mhs))

mhs_scores.sort(key=lambda x: x[1])

print(f"\nTotal docs with MHS: {len(mhs_scores)}")
print(f"Mean MHS: {sum(s for _, s in mhs_scores)/len(mhs_scores):.4f}")
print(f"\nWorst 20 MHS docs:")
for doc_id, score in mhs_scores[:20]:
    print(f"  {doc_id}: {score:.3f}")

print(f"\nBest 10 MHS docs:")
for doc_id, score in mhs_scores[-10:]:
    print(f"  {doc_id}: {score:.3f}")

# Distribution
buckets = {'< 0.5': 0, '0.5-0.7': 0, '0.7-0.8': 0, '0.8-0.9': 0, '>= 0.9': 0}
for _, s in mhs_scores:
    if s < 0.5: buckets['< 0.5'] += 1
    elif s < 0.7: buckets['0.5-0.7'] += 1
    elif s < 0.8: buckets['0.7-0.8'] += 1
    elif s < 0.9: buckets['0.8-0.9'] += 1
    else: buckets['>= 0.9'] += 1

print(f"\nDistribution:")
for k, v in buckets.items():
    print(f"  {k}: {v} docs")
