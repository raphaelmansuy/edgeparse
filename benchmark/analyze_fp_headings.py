"""Find docs with most false-positive headings."""
import os
import sys

sys.path.insert(0, "src")

gt_dir = "ground-truth/markdown"
pred_dir = "prediction/edgeparse/markdown"

results = []
for f in sorted(os.listdir(gt_dir)):
    if not f.endswith(".md"):
        continue
    doc_id = f[-7:-3]
    gt_f = os.path.join(gt_dir, f)
    pred_f = os.path.join(pred_dir, f)
    if not os.path.exists(pred_f):
        continue

    with open(gt_f) as g:
        gt_lines = g.readlines()
    with open(pred_f) as p:
        pred_lines = p.readlines()

    gt_headings = [l.strip() for l in gt_lines if l.startswith("#")]
    pred_headings = [l.strip() for l in pred_lines if l.startswith("#")]

    if len(pred_headings) > len(gt_headings) and len(gt_headings) <= 3:
        fp_count = len(pred_headings) - len(gt_headings)
        results.append(
            (doc_id, len(gt_headings), len(pred_headings), fp_count, gt_headings, pred_headings)
        )

results.sort(key=lambda x: -x[3])
print("Docs with most extra headings (GT<=3):")
for doc_id, gt_n, pred_n, fp, gt_h, pred_h in results[:15]:
    print(f"  Doc {doc_id}: GT={gt_n} Pred={pred_n} FP=+{fp}")
    for h in gt_h[:3]:
        print(f"    GT:   {h[:70]}")
    for h in pred_h[:5]:
        print(f"    PRED: {h[:70]}")
    print()
