"""Comprehensive TEDS analysis: show each doc's TEDS score, dims, and issue type."""
import os
import sys
sys.path.insert(0, 'src')
from evaluator_table import evaluate_table, extract_tables, TEDSEvaluator, calc_table_score, wrap_tables_in_html
from converter_markdown_table import convert_to_markdown_with_html_tables
from bs4 import BeautifulSoup

md_dir = 'prediction/edgeparse/markdown'
gt_dir = 'ground-truth/markdown'

results = []
for fname in sorted(os.listdir(gt_dir)):
    if not fname.endswith('.md'):
        continue
    doc_id = fname.replace('.md', '')
    gt_path = os.path.join(gt_dir, fname)
    pred_path = os.path.join(md_dir, fname)
    
    with open(gt_path) as f:
        gt = f.read()
    gt_html = convert_to_markdown_with_html_tables(gt)
    gt_tables = extract_tables(gt_html)
    if not gt_tables:
        continue
    
    if not os.path.exists(pred_path):
        results.append((doc_id, 0.0, 'missing_pred', [], []))
        continue
    
    with open(pred_path) as f:
        pred = f.read()
    pred_html = convert_to_markdown_with_html_tables(pred)
    pred_tables = extract_tables(pred_html)
    
    # Get dimensions
    def table_dims(tables):
        dims = []
        for t in tables:
            soup = BeautifulSoup(t, 'html.parser')
            rows = soup.find_all('tr')
            if rows:
                cols = max(len(r.find_all(['td', 'th'])) for r in rows)
                dims.append((len(rows), cols))
        return dims
    
    gt_dims = table_dims(gt_tables)
    pred_dims = table_dims(pred_tables)
    
    if not pred_tables:
        results.append((doc_id, 0.0, 'no_pred_tables', gt_dims, []))
        continue
    
    gt_data = wrap_tables_in_html(gt_tables)
    pred_data = wrap_tables_in_html(pred_tables)
    evaluator = TEDSEvaluator(structure_only=False)
    score = calc_table_score(gt_data, pred_data, evaluator)
    
    evaluator_s = TEDSEvaluator(structure_only=True)
    score_s = calc_table_score(gt_data, pred_data, evaluator_s)
    
    issue = 'good' if score >= 0.9 else ('close' if score >= 0.7 else 'low')
    
    results.append((doc_id, score, issue, gt_dims, pred_dims, score_s))

# Sort by score
results.sort(key=lambda x: x[1])

print(f"{'Doc':>20s}  {'TEDS':>6s}  {'TEDS-S':>6s}  {'GT dims':>18s}  {'Pred dims':>25s}  Issue")
print("-" * 100)
for r in results:
    if len(r) == 5:
        doc_id, score, issue, gt_dims, pred_dims = r
        score_s = 0.0
    else:
        doc_id, score, issue, gt_dims, pred_dims, score_s = r
    print(f"{doc_id:>20s}  {score:>6.3f}  {score_s:>6.3f}  {str(gt_dims):>18s}  {str(pred_dims):>25s}  {issue}")

print(f"\nMean TEDS: {sum(r[1] for r in results)/len(results):.4f}")
print(f"Low (<0.7): {sum(1 for r in results if r[1] < 0.7)}")
print(f"Close (0.7-0.9): {sum(1 for r in results if 0.7 <= r[1] < 0.9)}")
print(f"Good (>=0.9): {sum(1 for r in results if r[1] >= 0.9)}")
