"""Analyze TEDS failures for worst-performing documents."""
import sys
sys.path.insert(0, 'src')
from evaluator_table import evaluate_table, extract_tables
from converter_markdown_table import convert_to_markdown_with_html_tables
from bs4 import BeautifulSoup


def table_dims(html_str):
    soup = BeautifulSoup(html_str, 'html.parser')
    rows = soup.find_all('tr')
    cols = max((len(r.find_all(['td', 'th'])) for r in rows), default=0)
    return len(rows), cols


def main():
    worst_docs = ['122', '178', '132', '180', '200', '182', '146', '127', '089', '088']

    for doc_num in worst_docs:
        doc_id = f'01030000000{doc_num}'
        gt_path = f'ground-truth/markdown/{doc_id}.md'
        pred_path = f'prediction/edgeparse/markdown/{doc_id}.md'

        try:
            with open(gt_path) as f:
                gt_md = f.read()
            with open(pred_path) as f:
                pred_md = f.read()
        except FileNotFoundError:
            print(f"Doc {doc_num}: file not found")
            continue

        gt_html = convert_to_markdown_with_html_tables(gt_md)
        pred_html = convert_to_markdown_with_html_tables(pred_md)
        gt_tables = extract_tables(gt_html)
        pred_tables = extract_tables(pred_html)

        teds, teds_s = evaluate_table(gt_md, pred_md)

        gt_dims = [table_dims(t) for t in gt_tables]
        pred_dims = [table_dims(t) for t in pred_tables]

        print(f"Doc {doc_num}: TEDS={teds:.3f} GT={len(gt_tables)} tables {gt_dims} -> Pred={len(pred_tables)} tables {pred_dims}")


if __name__ == '__main__':
    main()
