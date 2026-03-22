"""Show GT vs Pred for worst MHS docs side by side."""
import sys
sys.path.insert(0, 'src')
from pathlib import Path

GT_DIR = Path("ground-truth/markdown")
PRED_DIR = Path("prediction/edgeparse/markdown")

docs = ["01030000000107", "01030000000148", "01030000000181", "01030000000103", "01030000000163"]

for doc_id in docs:
    gt = (GT_DIR / f"{doc_id}.md").read_text(encoding="utf-8")
    pred = (PRED_DIR / f"{doc_id}.md").read_text(encoding="utf-8")
    
    print(f"\n{'='*60}")
    print(f"DOC {doc_id}")
    print(f"{'='*60}")
    print("--- GT (first 20 lines) ---")
    for line in gt.split('\n')[:20]:
        print(f"  {line[:100]}")
    print("--- PRED (first 20 lines) ---")
    for line in pred.split('\n')[:20]:
        print(f"  {line[:100]}")
