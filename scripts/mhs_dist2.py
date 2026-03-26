#!/usr/bin/env python3
"""Analyze MHS score distribution after heading detector fixes."""
import sys, os, glob

bench = os.path.join(os.path.dirname(__file__), '..', 'benchmark')
sys.path.insert(0, os.path.join(bench, 'src'))
from evaluator_heading_level import evaluate_heading_level

gt_files   = sorted(glob.glob(os.path.join(bench, 'ground-truth/markdown/*.md')))
pred_files = sorted(glob.glob(os.path.join(bench, 'prediction/edgeparse/markdown/*.md')))

def extract_headings(md):
    return [l for l in md.splitlines() if l.strip().startswith('#')]

mhs_scores = []
for gf, pf in zip(gt_files, pred_files):
    gt_md  = open(gf).read()
    pr_md  = open(pf).read()
    doc_id = os.path.basename(gf).split('.')[0]
    gt_heads = extract_headings(gt_md)
    pr_heads = extract_headings(pr_md)
    if gt_heads:
        score, _ = evaluate_heading_level(gt_md, pr_md)
        if score is None:
            score = 0.0
        mhs_scores.append((score, doc_id, len(gt_heads), len(pr_heads)))

zero_mhs = [(s, d, g, p) for s, d, g, p in mhs_scores if s < 0.01]
low_mhs  = [(s, d, g, p) for s, d, g, p in mhs_scores if 0.01 <= s < 0.5]
high_mhs = [(s, d, g, p) for s, d, g, p in mhs_scores if s >= 0.5]
avg = sum(s for s, *_ in mhs_scores) / len(mhs_scores)

print(f"Total docs with GT headings: {len(mhs_scores)}")
print(f"MHS = 0 (<0.01):   {len(zero_mhs)}")
print(f"MHS in [0.01,0.5): {len(low_mhs)}")
print(f"MHS >= 0.5:        {len(high_mhs)}")
print(f"Average MHS:       {avg:.4f}")
print()
print("Zero/Near-zero MHS docs (gt_headings, pred_headings):")
for s, d, g, p in sorted(zero_mhs, key=lambda x: x[0])[:25]:
    print(f"  {d}: mhs={s:.3f}  gt_heads={g}  pred_heads={p}")
print()
print("Low MHS docs [0.01, 0.5):")
for s, d, g, p in sorted(low_mhs, key=lambda x: x[0])[:20]:
    print(f"  {d}: mhs={s:.3f}  gt_heads={g}  pred_heads={p}")
