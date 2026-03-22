"""Count docs with tables and TEDS distribution."""
import sys
sys.path.insert(0, 'src')
from pathlib import Path
from evaluator import _evaluate_single_document

GT_DIR = Path("ground-truth/markdown")
PRED_DIR = Path("prediction/edgeparse/markdown")

gt_files = sorted(GT_DIR.glob("*.md"))
results = []
for gt_file in gt_files:
    doc_id = gt_file.stem
    pred_file = PRED_DIR / f"{doc_id}.md"
    scores = _evaluate_single_document(doc_id, gt_file, pred_file)
    results.append(scores)

teds_docs = [(r.document_id, r.teds) for r in results if r.teds is not None]
print(f"Docs with tables (TEDS not None): {len(teds_docs)}")
print(f"TEDS distribution:")
for bucket in [1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0]:
    count = sum(1 for _, t in teds_docs if t >= bucket - 0.05 and t < bucket + 0.05)
    print(f"  ~{bucket:.1f}: {count}")

below_05 = [(d, t) for d, t in teds_docs if t < 0.5]
print(f"\nDocs with TEDS < 0.5 ({len(below_05)}):")
for d, t in sorted(below_05, key=lambda x: x[1]):
    print(f"  {d}: {t:.4f}")

avg = sum(t for _, t in teds_docs) / len(teds_docs)
print(f"\nAverage TEDS: {avg:.4f}")

# What if we fixed all < 0.5 docs to 0.8?
fixed = [(d, max(t, 0.8) if t < 0.5 else t) for d, t in teds_docs]
fixed_avg = sum(t for _, t in fixed) / len(fixed)
print(f"If <0.5 docs brought to 0.8: {fixed_avg:.4f} (+{fixed_avg - avg:.4f})")
