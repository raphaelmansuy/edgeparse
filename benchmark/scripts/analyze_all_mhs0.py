#!/usr/bin/env python3
"""Generate JSON for all MHS=0 docs and analyze font size patterns."""
import json
import os
import subprocess
from pathlib import Path
import sys

base_dir = '/Users/raphaelmansuy/Github/03-working/edgeparse'
benchmark_dir = f'{base_dir}/benchmark'
pred_dir = f'{benchmark_dir}/prediction/edgeparse'
binary = f'{base_dir}/target/release/edgeparse'
pdf_dir = f'{benchmark_dir}/benchmark/pdfs'  # wrong, let me fix
pdf_dir = f'{benchmark_dir}/pdfs'

# Load evaluation results to get MHS=0 docs
with open(f'{pred_dir}/evaluation.json') as f:
    data = json.load(f)

mhs_zero_docs = []
for doc in data['documents']:
    scores = doc.get('scores', {})
    mhs = scores.get('mhs', None)
    if mhs is not None and mhs == 0.0:
        mhs_zero_docs.append(doc['document_id'])

mhs_zero_docs.sort()
print(f"Total MHS=0 docs: {len(mhs_zero_docs)}")

# Generate JSON for all MHS=0 docs
output_dir = '/tmp/outall_mhs0'
os.makedirs(output_dir, exist_ok=True)

missing = []
for doc_id in mhs_zero_docs:
    json_path = f'{output_dir}/{doc_id}.json'
    if not os.path.exists(json_path):
        pdf_path = f'{pdf_dir}/{doc_id}.pdf'
        if os.path.exists(pdf_path):
            result = subprocess.run(
                [binary, pdf_path, '-f', 'json', '-q', '-o', output_dir],
                capture_output=True, timeout=30
            )
            if result.returncode != 0:
                missing.append(doc_id)
        else:
            missing.append(doc_id)

if missing:
    print(f"MISSING PDFs: {missing}")

# Now analyze font size distributions
print("\n=== Font Size Analysis for MHS=0 docs ===\n")
print(f"{'DocID':20s} {'body_mode':>10} {'heading_fs':>10} {'Gt_heading':30s} {'category'}")
print("-" * 90)

categories = {'footer': [], 'header': [], 'small_body': [], 'same_size': [], 'top': [], 'unknown': []}

gt_dir = f'{benchmark_dir}/ground-truth/markdown'

for doc_id in mhs_zero_docs:
    json_path = f'{output_dir}/{doc_id}.json'
    gt_path = f'{gt_dir}/{doc_id}.md'
    
    if not os.path.exists(json_path):
        continue
    
    with open(json_path) as f:
        d = json.load(f)
    
    gt_headings = []
    if os.path.exists(gt_path):
        with open(gt_path) as f:
            for line in f:
                if line.startswith('#'):
                    gt_headings.append(line.strip()[:40])
    
    kids = d.get('kids', [])
    
    # Collect font sizes of paragraphs
    para_sizes = []
    header_exists = False
    footer_exists = False
    
    for node in kids:
        t = node.get('type', '')
        if t == 'paragraph':
            size = node.get('font size', 0)
            if size and size > 0:
                para_sizes.append((size, node.get('content', '')[:30], 
                                   node.get('bounding box', [0,0,0,0])))
        elif t == 'header':
            header_exists = True
        elif t == 'heading':
            # EdgeParse detected some heading (might be footer)
            bb = node.get('bounding box', [0,0,0,0])
            y0 = bb[1] if len(bb) >= 2 else 0
            if y0 < 60:  # bottom of page
                footer_exists = True
    
    # Count font size occurrences
    size_counts = {}
    for sz, _, _ in para_sizes:
        key = round(sz * 10) / 10
        size_counts[key] = size_counts.get(key, 0) + 1
    
    # Find smallest-y and largest-y paragraphs
    min_y_para = min(para_sizes, key=lambda x: x[2][1] if len(x[2]) >= 2 else 999) if para_sizes else None
    max_y_para = max(para_sizes, key=lambda x: x[2][1] if len(x[2]) >= 2 else 0) if para_sizes else None
    
    # Categorize
    cat = 'unknown'
    if header_exists:
        cat = 'header_filtered'
    elif footer_exists:
        cat = 'footer_promoted'
    elif min_y_para and min_y_para[2][1] < 60:
        # Bottom paragraph is very low (footer text)
        cat = 'footer_text'
    elif max_y_para and max_y_para[2][1] > 700:
        # Top paragraph is very high (running header)
        cat = 'top_text'
    
    gt_h = gt_headings[0][:35] if gt_headings else 'N/A'
    
    top_y = max_y_para[2][1] if max_y_para and len(max_y_para[2]) >= 2 else 0
    top_fs = max_y_para[0] if max_y_para else 0
    
    # Most common (body) font size
    if size_counts:
        body_size = max(size_counts.items(), key=lambda x: x[1])[0]
    else:
        body_size = 0
    
    print(f"{doc_id:20s} {body_size:>10.2f} {top_fs:>10.2f} {top_y:>6.1f} {gt_h:35s} {cat}")
    
    categories[cat].append(doc_id) if cat in categories else categories['unknown'].append(doc_id)

print("\n=== CATEGORY SUMMARY ===")
for cat, docs in categories.items():
    print(f"  {cat}: {len(docs)} docs")
    for d in docs[:5]:
        print(f"    {d}")
