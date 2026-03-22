#!/usr/bin/env python3
"""Analyze heading over/under detection patterns across all docs."""
import json

with open("prediction/edgeparse/evaluation.json") as f:
    data = json.load(f)

with open("ground-truth/reference.json") as f:
    gt = json.load(f)

import os, re

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*)$", re.MULTILINE)

# Collect GT heading counts
gt_counts = {}
for doc_key, doc in gt.items():
    doc_id = doc_key.replace(".pdf", "")
    count = sum(1 for el in doc.get("elements", []) if "Heading" in el.get("category", "") or el.get("category", "") == "Title")
    gt_counts[doc_id] = count

# Analyze each doc
over_detected = []
under_detected = []
exact = []
wrong_text = []

for doc in data["documents"]:
    doc_id = doc["document_id"]
    mhs = doc["scores"].get("mhs")
    if mhs is None:
        continue
    
    gt_count = gt_counts.get(doc_id, 0)
    if gt_count == 0:
        continue
    
    md_path = f"prediction/edgeparse/markdown/{doc_id}.md"
    if not os.path.exists(md_path):
        continue
    with open(md_path) as f:
        md = f.read()
    pred_count = len(HEADING_RE.findall(md))
    
    diff = pred_count - gt_count
    if diff > 0:
        over_detected.append((doc_id, mhs, gt_count, pred_count, diff))
    elif diff < 0:
        under_detected.append((doc_id, mhs, gt_count, pred_count, diff))
    else:
        exact.append((doc_id, mhs, gt_count, pred_count))

# Sort by MHS (worst first)
over_detected.sort(key=lambda x: x[1])
under_detected.sort(key=lambda x: x[1])
exact.sort(key=lambda x: x[1])

print(f"=== OVER-DETECTED: {len(over_detected)} docs (pred > GT) ===")
print(f"Mean MHS: {sum(x[1] for x in over_detected)/max(1,len(over_detected)):.4f}")
for doc_id, mhs, gt_c, pred_c, diff in over_detected[:15]:
    print(f"  {doc_id}: MHS={mhs:.4f}, GT={gt_c}, Pred={pred_c}, Extra=+{diff}")

print(f"\n=== UNDER-DETECTED: {len(under_detected)} docs (pred < GT) ===")
print(f"Mean MHS: {sum(x[1] for x in under_detected)/max(1,len(under_detected)):.4f}")
for doc_id, mhs, gt_c, pred_c, diff in under_detected[:15]:
    print(f"  {doc_id}: MHS={mhs:.4f}, GT={gt_c}, Pred={pred_c}, Missing={diff}")

print(f"\n=== EXACT MATCH: {len(exact)} docs (pred == GT) ===")
print(f"Mean MHS: {sum(x[1] for x in exact)/max(1,len(exact)):.4f}")
for doc_id, mhs, gt_c, pred_c in exact[:10]:
    print(f"  {doc_id}: MHS={mhs:.4f}, GT={gt_c}, Pred={pred_c}")

# Impact analysis: if we could fix all over-detected to pred==GT
total_mhs = sum(d["scores"]["mhs"] for d in data["documents"] if d["scores"].get("mhs") is not None)
count_mhs = sum(1 for d in data["documents"] if d["scores"].get("mhs") is not None)
print(f"\nOverall MHS: {total_mhs/count_mhs:.4f}")

# Potential MHS gain from fixing over-detection
print("\nPotential from fixing over-detection (+MHS if each doc reaches avg MHS):")
avg_mhs = total_mhs / count_mhs
for doc_id, mhs, gt_c, pred_c, diff in over_detected[:10]:
    potential = (avg_mhs - mhs) / count_mhs
    print(f"  {doc_id}: current={mhs:.4f}, potential gain={potential:.4f} (extra {diff} headings)")
