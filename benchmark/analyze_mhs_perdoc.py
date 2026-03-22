#!/usr/bin/env python3
"""Analyze per-doc MHS scores to find worst docs and improvement targets."""
import json, os, sys
sys.path.insert(0, 'src')
from evaluator_heading_level import evaluate_heading_level

with open('ground-truth/reference.json') as f:
    gt = json.load(f)

pred_dir = 'prediction/edgeparse/markdown'
docling_dir = 'prediction/docling/markdown'

mhs_docs = []
for fname in sorted(os.listdir(pred_dir)):
    if not fname.endswith('.md'):
        continue
    doc_id = fname.replace('.md', '.pdf')
    if doc_id not in gt:
        continue
    gt_doc = gt[doc_id]
    gt_headings = [(e.get('level', 1), e.get('value', ''))
                   for e in gt_doc.get('elements', []) if e.get('type') == 'heading']
    if not gt_headings:
        continue
    with open(os.path.join(pred_dir, fname)) as f:
        md = f.read()
    score = evaluate_heading_level(md, gt_headings)

    # Also get docling score if available
    docling_score = None
    docling_path = os.path.join(docling_dir, fname)
    if os.path.exists(docling_path):
        with open(docling_path) as f:
            docling_md = f.read()
        docling_score = evaluate_heading_level(docling_md, gt_headings)

    mhs_docs.append((doc_id, score, docling_score, gt_headings))

mhs_docs.sort(key=lambda x: x[1])
print(f'Total MHS docs: {len(mhs_docs)}')
print(f'\nWorst 30 MHS docs (ours vs docling):')
for doc_id, mhs, docling_mhs, gt_h in mhs_docs[:30]:
    gap = (docling_mhs - mhs) if docling_mhs is not None else 0
    docling_str = f'{docling_mhs:.4f}' if docling_mhs is not None else 'N/A'
    print(f'  {doc_id}: EP={mhs:.4f} DOC={docling_str} gap={gap:+.4f} ({len(gt_h)} GT headings)')

print(f'\nDocs where we lose ≥0.3 to docling on MHS:')
big_gap = [(d, m, dm, gh) for d, m, dm, gh in mhs_docs if dm is not None and dm - m >= 0.3]
big_gap.sort(key=lambda x: x[2] - x[1], reverse=True)
for doc_id, mhs, docling_mhs, gt_h in big_gap:
    print(f'  {doc_id}: EP={mhs:.4f} DOC={docling_mhs:.4f} gap={docling_mhs-mhs:+.4f}')
    # Show GT headings
    for lvl, val in gt_h[:5]:
        print(f'    L{lvl}: {val[:60]}')
    if len(gt_h) > 5:
        print(f'    ... {len(gt_h)-5} more')
