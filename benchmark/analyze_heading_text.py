"""Analyze heading text mismatches between GT and predictions."""
import os
import re
from rapidfuzz.distance import Levenshtein

gt_dir = "ground-truth/markdown"
pred_dir = "prediction/edgeparse/markdown"
heading_re = re.compile(r"^(#{1,6})\s+(.*)$", re.MULTILINE)

mismatches = []
for f in sorted(os.listdir(gt_dir)):
    if not f.endswith(".md"):
        continue
    doc_id = f.replace(".md", "")
    gt = open(os.path.join(gt_dir, f)).read()
    gt_h = [m[1].strip() for m in heading_re.findall(gt)]
    if not gt_h:
        continue
    pred_path = os.path.join(pred_dir, f)
    if not os.path.exists(pred_path):
        continue
    pred = open(pred_path).read()
    pred_h = [m[1].strip() for m in heading_re.findall(pred)]
    if not pred_h:
        continue

    for gh in gt_h:
        best_dist = float("inf")
        best_ph = None
        for ph in pred_h:
            dist = Levenshtein.distance(gh, ph) / max(len(gh), len(ph), 1)
            if dist < best_dist:
                best_dist = dist
                best_ph = ph
        if 0 < best_dist < 1.0:
            mismatches.append((doc_id, gh[:80], best_ph[:80] if best_ph else "", best_dist))

mismatches.sort(key=lambda x: -x[3])
print(f"Total heading text mismatches: {len(mismatches)}")
print()
for doc_id, gt, pred, dist in mismatches[:25]:
    print(f"  {doc_id}: dist={dist:.3f}")
    print(f"    GT:   {gt}")
    print(f"    Pred: {pred}")
    print()
