#!/usr/bin/env python3
"""Analyze per-document scores across all metrics to find improvement opportunities."""

import json
from pathlib import Path

eval_path = Path(__file__).parent / "prediction" / "edgeparse" / "evaluation.json"
with open(eval_path) as f:
    data = json.load(f)

docs = data["documents"]

# Collect per-metric scores
nid_scores = []
teds_scores = []
mhs_scores = []
sbf_scores = []
overall_scores = []

for doc in docs:
    did = doc["document_id"]
    s = doc["scores"]
    nid = s.get("nid")
    teds = s.get("teds")
    mhs = s.get("mhs")
    sbf = s.get("prose_block_boundary_f1")
    ov = s.get("overall")
    
    if nid is not None:
        nid_scores.append((did, nid))
    if teds is not None:
        teds_scores.append((did, teds))
    if mhs is not None:
        mhs_scores.append((did, mhs))
    if sbf is not None:
        sbf_scores.append((did, sbf))
    if ov is not None:
        overall_scores.append((did, ov))

# Sort by score ascending (worst first)
nid_scores.sort(key=lambda x: x[1])
teds_scores.sort(key=lambda x: x[1])
mhs_scores.sort(key=lambda x: x[1])
sbf_scores.sort(key=lambda x: x[1])
overall_scores.sort(key=lambda x: x[1])

print(f"=== NID (n={len(nid_scores)}, mean={sum(s for _,s in nid_scores)/len(nid_scores):.4f}) ===")
print("Worst 20:")
for did, score in nid_scores[:20]:
    print(f"  {did}: {score:.4f}")

print(f"\n=== TEDS (n={len(teds_scores)}, mean={sum(s for _,s in teds_scores)/len(teds_scores):.4f}) ===")
print("Worst 20:")
for did, score in teds_scores[:20]:
    print(f"  {did}: {score:.4f}")

print(f"\n=== MHS (n={len(mhs_scores)}, mean={sum(s for _,s in mhs_scores)/len(mhs_scores):.4f}) ===")
print("Worst 20:")
for did, score in mhs_scores[:20]:
    print(f"  {did}: {score:.4f}")

print(f"\n=== SBF (n={len(sbf_scores)}, mean={sum(s for _,s in sbf_scores)/len(sbf_scores):.4f}) ===")
print("Worst 20:")
for did, score in sbf_scores[:20]:
    print(f"  {did}: {score:.4f}")

print(f"\n=== Overall (n={len(overall_scores)}, mean={sum(s for _,s in overall_scores)/len(overall_scores):.4f}) ===")
print("Worst 20:")
for did, score in overall_scores[:20]:
    print(f"  {did}: {score:.4f}")

# Distribution analysis
print("\n=== MHS Distribution ===")
bins = [0, 0.2, 0.4, 0.6, 0.7, 0.8, 0.9, 1.001]
for i in range(len(bins)-1):
    count = sum(1 for _, s in mhs_scores if bins[i] <= s < bins[i+1])
    print(f"  [{bins[i]:.1f}, {bins[i+1]:.1f}): {count}")

print("\n=== NID Distribution ===")
for i in range(len(bins)-1):
    count = sum(1 for _, s in nid_scores if bins[i] <= s < bins[i+1])
    print(f"  [{bins[i]:.1f}, {bins[i+1]:.1f}): {count}")

# How much would fixing the worst docs improve mean?
print("\n=== MHS: Impact of improving worst docs ===")
mhs_mean = sum(s for _, s in mhs_scores) / len(mhs_scores)
for target in [0.5, 0.6, 0.7]:
    improved = [(did, max(s, target)) for did, s in mhs_scores]
    new_mean = sum(s for _, s in improved) / len(improved)
    print(f"  Raising all below {target:.1f} to {target:.1f}: mean {mhs_mean:.4f} -> {new_mean:.4f} (+{new_mean-mhs_mean:.4f})")

print("\n=== NID: Impact of improving worst docs ===")
nid_mean = sum(s for _, s in nid_scores) / len(nid_scores)
for target in [0.7, 0.8, 0.9]:
    improved = [(did, max(s, target)) for did, s in nid_scores]
    new_mean = sum(s for _, s in improved) / len(improved)
    print(f"  Raising all below {target:.1f} to {target:.1f}: mean {nid_mean:.4f} -> {new_mean:.4f} (+{new_mean-nid_mean:.4f})")
