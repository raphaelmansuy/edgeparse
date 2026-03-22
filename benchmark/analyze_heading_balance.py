"""Analyze heading over/under-detection across all docs."""
import sys, re
sys.path.insert(0, 'src')
from pathlib import Path

GT_DIR = Path("ground-truth/markdown")
PRED_DIR = Path("prediction/edgeparse/markdown")

def count_headings(text):
    count = 0
    for line in text.split('\n'):
        if re.match(r'^#{1,6}\s+', line):
            count += 1
    return count

gt_files = sorted(GT_DIR.glob("*.md"))
over_detected = []  # pred > gt
under_detected = []  # pred < gt
matched = []

for gt_file in gt_files:
    doc_id = gt_file.stem
    pred_file = PRED_DIR / f"{doc_id}.md"
    if not pred_file.exists():
        continue
    gt_md = gt_file.read_text(encoding="utf-8")
    pred_md = pred_file.read_text(encoding="utf-8")
    
    gt_h = count_headings(gt_md)
    pred_h = count_headings(pred_md)
    
    if pred_h > gt_h:
        over_detected.append((doc_id, gt_h, pred_h, pred_h - gt_h))
    elif pred_h < gt_h:
        under_detected.append((doc_id, gt_h, pred_h, gt_h - pred_h))
    else:
        matched.append((doc_id, gt_h, pred_h))

print(f"Total docs: {len(over_detected) + len(under_detected) + len(matched)}")
print(f"Exact match: {len(matched)} docs")
print(f"Over-detected: {len(over_detected)} docs (pred > gt)")
print(f"Under-detected: {len(under_detected)} docs (pred < gt)")

print(f"\n=== OVER-DETECTED (worst first) ===")
over_detected.sort(key=lambda x: -x[3])
for doc_id, gt_h, pred_h, diff in over_detected[:15]:
    print(f"  {doc_id}: GT={gt_h} Pred={pred_h} (EXTRA +{diff})")

print(f"\n=== UNDER-DETECTED (worst first) ===")
under_detected.sort(key=lambda x: -x[3])
for doc_id, gt_h, pred_h, diff in under_detected[:15]:
    print(f"  {doc_id}: GT={gt_h} Pred={pred_h} (MISSING -{diff})")

# Sum total extra and total missing
total_extra = sum(d for _, _, _, d in over_detected)
total_missing = sum(d for _, _, _, d in under_detected)
print(f"\nTotal extra headings: {total_extra}")
print(f"Total missing headings: {total_missing}")
