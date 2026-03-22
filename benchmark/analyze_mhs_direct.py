"""Get per-doc MHS scores by running the evaluator directly."""
import os
import sys
sys.path.insert(0, 'src')
from evaluator_heading_level import evaluate_heading_level

gt_dir = 'ground-truth/markdown'
pred_dir = 'prediction/edgeparse/markdown'

scores = []
for fname in sorted(os.listdir(gt_dir)):
    if not fname.endswith('.md'):
        continue
    doc_id = fname.replace('.md', '')
    pred_path = os.path.join(pred_dir, fname)
    gt_path = os.path.join(gt_dir, fname)
    
    if not os.path.exists(pred_path):
        continue
    
    with open(gt_path) as f:
        gt_text = f.read()
    with open(pred_path) as f:
        pred_text = f.read()
    
    # Check if GT has headings
    gt_has_headings = any(line.startswith('#') for line in gt_text.split('\n') if line.strip())
    if not gt_has_headings:
        continue
    
    try:
        result = evaluate_heading_level(gt_text, pred_text)
        if result is not None:
            score = result[0] if isinstance(result, tuple) else result
            scores.append((doc_id, score))
    except Exception as e:
        pass

scores.sort(key=lambda x: x[1])
print(f"Total docs with MHS: {len(scores)}")
print(f"Mean MHS: {sum(s for _, s in scores)/len(scores):.4f}")

print(f"\nWorst 30 MHS docs:")
for doc_id, score in scores[:30]:
    print(f"  {doc_id}: {score:.3f}")

print(f"\nDocs scoring 0.5-0.7:")
for doc_id, score in scores:
    if 0.5 <= score < 0.7:
        print(f"  {doc_id}: {score:.3f}")
