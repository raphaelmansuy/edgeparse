"""Show predicted headings for over-detected docs."""
import sys, re
sys.path.insert(0, 'src')
from pathlib import Path

GT_DIR = Path("ground-truth/markdown")
PRED_DIR = Path("prediction/edgeparse/markdown")

def get_headings(text):
    headings = []
    for line in text.split('\n'):
        m = re.match(r'^(#{1,6})\s+(.+)', line)
        if m:
            headings.append((len(m.group(1)), m.group(2).strip()))
    return headings

# Focus on worst over-detected docs
docs = ["01030000000170", "01030000000043", "01030000000200", "01030000000144",
        "01030000000085", "01030000000086", "01030000000190",
        "01030000000008", "01030000000030", "01030000000075",
        "01030000000081", "01030000000095", "01030000000119"]

for doc_id in docs:
    gt_file = GT_DIR / f"{doc_id}.md"
    pred_file = PRED_DIR / f"{doc_id}.md"
    if not pred_file.exists():
        continue
    gt_h = get_headings(gt_file.read_text(encoding="utf-8"))
    pred_h = get_headings(pred_file.read_text(encoding="utf-8"))
    
    print(f"\n=== {doc_id} GT={len(gt_h)} Pred={len(pred_h)} ===")
    if gt_h:
        print(f"  GT: {gt_h}")
    else:
        print(f"  GT: (none)")
    print(f"  Pred:")
    for level, text in pred_h:
        print(f"    H{level}: {text[:80]}")
