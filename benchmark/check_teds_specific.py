"""Check TEDS for specific fragmented docs."""
import sys
sys.path.insert(0, 'src')
from evaluator_table import evaluate_table, extract_tables
from converter_markdown_table import convert_to_markdown_with_html_tables
from bs4 import BeautifulSoup


def dims(t):
    s = BeautifulSoup(t, 'html.parser')
    rows = s.find_all('tr')
    cols = max((len(r.find_all(['td', 'th'])) for r in rows), default=0)
    return len(rows), cols


docs = ['188', '078', '047', '046', '116', '170', '197']
for d in docs:
    doc_id = f'01030000000{d}'
    with open(f'ground-truth/markdown/{doc_id}.md') as f:
        gt = f.read()
    with open(f'prediction/edgeparse/markdown/{doc_id}.md') as f:
        pred = f.read()
    gt_html = convert_to_markdown_with_html_tables(gt)
    pred_html = convert_to_markdown_with_html_tables(pred)
    gt_tables = extract_tables(gt_html)
    pred_tables = extract_tables(pred_html)
    teds, _ = evaluate_table(gt, pred)
    gt_dims = [dims(t) for t in gt_tables]
    pred_dims = [dims(t) for t in pred_tables]
    print(f'Doc {d}: TEDS={teds:.3f} GT={gt_dims} Pred={pred_dims}')
