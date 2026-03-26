#!/usr/bin/env python3
"""Analyze MHS score distribution after heading detector fixes."""
import json, glob, sys, os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'benchmark', 'src'))
from evaluator_heading_level import evaluate_heading_level

def blocks_to_markdown(blocks):
    """Convert block list to markdown string for heading evaluation."""
    lines = []
    for b in blocks:
        t = b.get('type', '')
        if t == 'heading':
            level = b.get('level', 1)
            lines.append('#' * level + ' ' + b.get('text', ''))
        elif t == 'paragraph':
            lines.append(b.get('text', ''))
    return '\n'.join(lines)

benchmark = os.path.join(os.path.dirname(__file__), '..', 'benchmark')
pred_files = sorted(glob.glob(os.path.join(benchmark, 'prediction/edgeparse/*.json')))
gt_files   = sorted(glob.glob(os.path.join(benchmark, 'ground-truth/*.json')))

mhs_scores = []
for gf, pf in zip(gt_files, pred_files):
    gt = json.load(open(gf))
    pr = json.load(open(pf))
    doc_id = os.path.basename(gf).split('.')[0]
    gt_heads = [b for b in gt.get('blocks', []) if b.get('type') == 'heading']
    pr_heads = [b for b in pr.get('blocks', []) if b.get('type') == 'heading']
    if gt_heads:
        gt_md = gt.get('markdown', blocks_to_markdown(gt.get('blocks', [])))
        pr_md = pr.get('markdown', blocks_to_markdown(pr.get('blocks', [])))
        score, _ = evaluate_heading_level(gt_md, pr_md)
        if score is None:
            score = 0.0
        mhs_scores.append((score, doc_id, len(gt_heads), len(pr_heads)))

zero_mhs = [(s, d, g, p) for s, d, g, p in mhs_scores if s < 0.01]
low_mhs  = [(s, d, g, p) for s, d, g, p in mhs_scores if 0.01 <= s < 0.5]
high_mhs = [(s, d, g, p) for s, d, g, p in mhs_scores if s >= 0.5]

print(f"Total docs with GT headings: {len(mhs_scores)}")
print(f"MHS = 0:        {len(zero_mhs)}")
print(f"MHS in (0,0.5): {len(low_mhs)}")
print(f"MHS >= 0.5:     {len(high_mhs)}")
print(f"Average MHS:    {sum(s for s, *_ in mhs_scores) / len(mhs_scores):.4f}")
print()
print("Zero/Near-zero MHS docs (GT heads, pred heads):")
for s, d, g, p in sorted(zero_mhs, key=lambda x: x[0])[:20]:
    print(f"  {d}: mhs={s:.3f}  gt={g}  pred={p}")
print()
print("Low MHS docs (0, 0.5):")
for s, d, g, p in sorted(low_mhs, key=lambda x: x[0])[:20]:
    print(f"  {d}: mhs={s:.3f}  gt={g}  pred={p}")
