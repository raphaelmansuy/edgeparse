"""Debug TEDS for doc 188 — compare row content."""
import sys
sys.path.insert(0, 'src')
from evaluator_table import evaluate_table, extract_tables, TEDSEvaluator, calc_table_score, wrap_tables_in_html
from converter_markdown_table import convert_to_markdown_with_html_tables
from bs4 import BeautifulSoup


doc_id = '01030000000188'
with open(f'ground-truth/markdown/{doc_id}.md') as f:
    gt = f.read()
with open(f'prediction/edgeparse/markdown/{doc_id}.md') as f:
    pred = f.read()

gt_html = convert_to_markdown_with_html_tables(gt)
pred_html = convert_to_markdown_with_html_tables(pred)
gt_tables = extract_tables(gt_html)
pred_tables = extract_tables(pred_html)

print(f"GT tables: {len(gt_tables)}, Pred tables: {len(pred_tables)}")

# Show rows from each
for i, t in enumerate(gt_tables):
    soup = BeautifulSoup(t, 'html.parser')
    rows = soup.find_all('tr')
    print(f"\nGT Table {i}: {len(rows)} rows")
    for j, row in enumerate(rows[:3]):
        cells = [c.get_text(strip=True) for c in row.find_all(['td', 'th'])]
        print(f"  Row {j}: {cells[:3]}...")

for i, t in enumerate(pred_tables):
    soup = BeautifulSoup(t, 'html.parser')
    rows = soup.find_all('tr')
    print(f"\nPred Table {i}: {len(rows)} rows")
    for j, row in enumerate(rows[:3]):
        cells = [c.get_text(strip=True) for c in row.find_all(['td', 'th'])]
        print(f"  Row {j}: {cells[:3]}...")

# Show individual TEDS per table pair
print("\n--- TEDS calculation ---")
print(f"GT combined: {len(gt_tables)} tables")
print(f"Pred combined: {len(pred_tables)} tables")

gt_data = wrap_tables_in_html(gt_tables)
pred_data = wrap_tables_in_html(pred_tables)

evaluator = TEDSEvaluator(structure_only=False)
score = calc_table_score(gt_data, pred_data, evaluator)
print(f"Combined TEDS: {score:.3f}")

evaluator_s = TEDSEvaluator(structure_only=True)
score_s = calc_table_score(gt_data, pred_data, evaluator_s)
print(f"Combined TEDS-S: {score_s:.3f}")
