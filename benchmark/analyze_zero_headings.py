#!/usr/bin/env python3
"""Find docs that have zero predicted headings but non-zero GT headings, then 
analyze how many heading texts appear in the markdown prediction."""
import json
import os

with open("prediction/edgeparse/evaluation.json") as f:
    data = json.load(f)

with open("ground-truth/reference.json") as f:
    gt = json.load(f)

# Get GT headings per doc
gt_headings = {}
for doc_key, doc in gt.items():
    doc_id = doc_key.replace(".pdf", "")
    headings = []
    for el in doc.get("elements", []):
        cat = el.get("category", "")
        if "Heading" in cat or cat == "Title":
            text = el.get("content", {}).get("text", "")
            if text:
                headings.append(text)
    if headings:
        gt_headings[doc_id] = headings

# Find docs with zero predicted headings
for doc in data["documents"]:
    doc_id = doc["document_id"]
    mhs = doc["scores"].get("mhs")
    if mhs is None:
        continue
    
    md_path = f"prediction/edgeparse/markdown/{doc_id}.md"
    if not os.path.exists(md_path):
        continue
    
    with open(md_path) as f:
        md = f.read()
    
    # Count predicted headings
    pred_count = sum(1 for line in md.split("\n") if line.startswith("#"))
    gt_h = gt_headings.get(doc_id, [])
    
    if pred_count == 0 and gt_h:
        print(f"\n{doc_id}: MHS={mhs:.4f}, pred=0, gt={len(gt_h)}")
        for h in gt_h:
            # Check if GT heading text appears in the markdown
            found = h[:30].lower() in md.lower()
            status = "FOUND" if found else "MISSING"
            print(f"  [{status}] \"{h[:80]}\" ({len(h.split())} words, {len(h)} chars)")
