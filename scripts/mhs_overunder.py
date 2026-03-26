#!/usr/bin/env python3
"""Analyze over-detection and under-detection for MHS."""
import sys, os, glob

bench = os.path.join(os.path.dirname(__file__), '..', 'benchmark')
sys.path.insert(0, os.path.join(bench, 'src'))
from evaluator_heading_level import evaluate_heading_level

gt_dir = os.path.join(bench, 'ground-truth/markdown')
pr_dir = os.path.join(bench, 'prediction/edgeparse/markdown')

docs = [(os.path.basename(f).split('.')[0], f) for f in sorted(glob.glob(gt_dir + '/*.md'))]

results = []
for doc_id, gf in docs:
    pf = os.path.join(pr_dir, doc_id + '.md')
    if not os.path.exists(pf):
        continue
    gt = open(gf).read()
    pr = open(pf).read()
    gt_h = [l for l in gt.splitlines() if l.strip().startswith('#')]
    pr_h = [l for l in pr.splitlines() if l.strip().startswith('#')]
    if gt_h:
        score, _ = evaluate_heading_level(gt, pr)
        if score is None:
            score = 0.0
        results.append((score, doc_id, len(gt_h), len(pr_h)))

# Over-detection: pred significantly more than GT
over = [(s, d, g, p) for s, d, g, p in results if p > g + 2]
print("Over-detection docs (pred > gt+2):")
for s, d, g, p in sorted(over, key=lambda x: x[0]):
    print(f"  {d}: mhs={s:.3f}  gt={g}  pred={p}")
print()

# Under-detection: pred < gt and low MHS
under = [(s, d, g, p) for s, d, g, p in results if p < g and s < 0.5]
print("Under-detection docs (pred < gt, mhs < 0.5):")
for s, d, g, p in sorted(under, key=lambda x: x[0])[:20]:
    print(f"  {d}: mhs={s:.3f}  gt={g}  pred={p}")
