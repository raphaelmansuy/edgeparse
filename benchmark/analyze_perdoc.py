"""Per-document score analysis for edgeparse."""
import sys
sys.path.insert(0, 'src')

from pathlib import Path
from evaluator import _evaluate_single_document

GT_DIR = Path("ground-truth/markdown")
PRED_DIR = Path("prediction/edgeparse/markdown")

# Get all ground truth docs
gt_files = sorted(GT_DIR.glob("*.md"))
results = []

for gt_file in gt_files:
    doc_id = gt_file.stem
    pred_file = PRED_DIR / f"{doc_id}.md"
    scores = _evaluate_single_document(doc_id, gt_file, pred_file)
    results.append(scores)

# Sort by TEDS
print("=== WORST TEDS DOCS (table structure) ===")
teds_sorted = sorted([r for r in results if r.teds is not None], key=lambda x: x.teds)
for r in teds_sorted[:15]:
    print(f"  {r.document_id}: TEDS={r.teds:.4f}")

print()
print("=== WORST MHS DOCS (heading hierarchy) ===")
mhs_sorted = sorted([r for r in results if r.mhs is not None], key=lambda x: x.mhs)
for r in mhs_sorted[:15]:
    print(f"  {r.document_id}: MHS={r.mhs:.4f}")

print()
print("=== WORST PBF DOCS (paragraph boundaries) ===")
pbf_sorted = sorted([r for r in results if r.paragraph_boundary_f1 is not None], key=lambda x: x.paragraph_boundary_f1)
for r in pbf_sorted[:15]:
    print(f"  {r.document_id}: PBF={r.paragraph_boundary_f1:.4f}")

print()
print("=== WORST NID DOCS (reading order) ===")
nid_sorted = sorted([r for r in results if r.nid is not None], key=lambda x: x.nid)
for r in nid_sorted[:15]:
    print(f"  {r.document_id}: NID={r.nid:.4f}")

# Summary
print()
print(f"Total docs: {len(results)}")
print(f"TEDS < 0.5: {sum(1 for r in results if r.teds is not None and r.teds < 0.5)}")
print(f"MHS == 0.0: {sum(1 for r in results if r.mhs is not None and r.mhs == 0.0)}")
print(f"MHS < 0.5: {sum(1 for r in results if r.mhs is not None and r.mhs < 0.5)}")
print(f"PBF < 0.5: {sum(1 for r in results if r.paragraph_boundary_f1 is not None and r.paragraph_boundary_f1 < 0.5)}")
print(f"NID < 0.8: {sum(1 for r in results if r.nid is not None and r.nid < 0.8)}")
