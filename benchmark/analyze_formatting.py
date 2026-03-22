#!/usr/bin/env python3
"""Find systematic text formatting differences between edgeparse and ground truth."""
import os, re, csv

ep_dir = "prediction/edgeparse/markdown"
gt_dir = "ground-truth/markdown"

scores = {}
with open("prediction/edgeparse/evaluation.csv") as f:
    for row in csv.DictReader(f):
        doc_id = row['document_id'].lstrip("'")
        nid = float(row['nid']) if row['nid'] else None
        scores[doc_id] = nid

# Analyze formatting patterns for docs with NID 0.9-0.99
patterns = {
    'extra_heading_markers': 0,  # Count of docs with different heading counts
    'extra_blank_lines': 0,
    'missing_content': 0,
    'hyphen_breaks': 0,
    'table_differences': 0,
}

# Check specific formatting patterns
nid_docs = [(d, s) for d, s in scores.items() if s is not None and 0.85 < s < 0.99]
nid_docs.sort(key=lambda x: x[1])

print(f"Analyzing {len(nid_docs)} docs with NID in [0.85, 0.99)")
print()

for doc_id, nid in nid_docs[:20]:
    ep_path = os.path.join(ep_dir, f"{doc_id}.md")
    gt_path = os.path.join(gt_dir, f"{doc_id}.md")
    if not os.path.exists(ep_path) or not os.path.exists(gt_path):
        continue
    
    with open(ep_path) as f:
        ep_text = f.read()
    with open(gt_path) as f:
        gt_text = f.read()
    
    # Count headings
    ep_headings = len(re.findall(r'^#{1,6}\s', ep_text, re.MULTILINE))
    gt_headings = len(re.findall(r'^#{1,6}\s', gt_text, re.MULTILINE))
    
    # Count pipe tables
    ep_tables = len(re.findall(r'^\|.+\|$', ep_text, re.MULTILINE))
    gt_tables = len(re.findall(r'^\|.+\|$', gt_text, re.MULTILINE))
    
    # Count HTML tables
    gt_html_tables = len(re.findall(r'<table', gt_text, re.IGNORECASE))
    
    # Text length
    ep_words = len(ep_text.split())
    gt_words = len(gt_text.split())
    word_ratio = ep_words / gt_words if gt_words > 0 else 0
    
    # Hyphenated words at line ends in GT
    gt_hyphens = len(re.findall(r'\w-\n\w', gt_text))
    
    print(f"  {doc_id}: NID={nid:.4f} EP_words={ep_words} GT_words={gt_words} ratio={word_ratio:.2f} "
          f"EP_h={ep_headings} GT_h={gt_headings} GT_htmltbl={gt_html_tables} GT_hyphens={gt_hyphens}")

# Also look at near-perfect docs (0.99-1.0) 
print(f"\n=== Docs with NID 0.99-1.0 ===")
near_perfect = [(d, s) for d, s in scores.items() if s is not None and 0.99 <= s < 1.0]
near_perfect.sort(key=lambda x: x[1])
for doc_id, nid in near_perfect[:10]:
    ep_path = os.path.join(ep_dir, f"{doc_id}.md")
    gt_path = os.path.join(gt_dir, f"{doc_id}.md")
    if not os.path.exists(ep_path) or not os.path.exists(gt_path):
        continue
    with open(ep_path) as f:
        ep_text = f.read()
    with open(gt_path) as f:
        gt_text = f.read()
    ep_words = len(ep_text.split())
    gt_words = len(gt_text.split())
    ep_headings = len(re.findall(r'^#{1,6}\s', ep_text, re.MULTILINE))
    gt_headings = len(re.findall(r'^#{1,6}\s', gt_text, re.MULTILINE))
    print(f"  {doc_id}: NID={nid:.4f} EP_words={ep_words} GT_words={gt_words} EP_h={ep_headings} GT_h={gt_headings}")
