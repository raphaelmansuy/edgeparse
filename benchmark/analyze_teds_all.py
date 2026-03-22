"""Comprehensive analysis of all TEDS documents."""
import sys
sys.path.insert(0, 'src')
from evaluator_table import evaluate_table, extract_tables
from converter_markdown_table import convert_to_markdown_with_html_tables
from bs4 import BeautifulSoup
import json
from pathlib import Path


def table_dims(html_str):
    soup = BeautifulSoup(html_str, 'html.parser')
    rows = soup.find_all('tr')
    cols = max((len(r.find_all(['td', 'th'])) for r in rows), default=0)
    return len(rows), cols


def main():
    # Get all docs from the latest evaluation
    eval_path = Path('prediction/edgeparse/evaluation.json')
    with open(eval_path) as f:
        eval_data = json.load(f)
    
    docs = eval_data.get('documents', [])
    teds_docs = [(d['document_id'], d['scores']['teds']) for d in docs if d['scores'].get('teds') is not None]
    teds_docs.sort(key=lambda x: x[1])
    
    print(f"Total TEDS docs: {len(teds_docs)}")
    print()
    
    categories = {'missing_pred': [], 'fragmented': [], 'extra_rows': [], 'missing_rows_cols': [], 'good': [], 'close': []}
    
    for doc_id, teds_score in teds_docs:
        gt_path = f'ground-truth/markdown/{doc_id}.md'
        pred_path = f'prediction/edgeparse/markdown/{doc_id}.md'
        
        try:
            with open(gt_path) as f:
                gt_md = f.read()
            with open(pred_path) as f:
                pred_md = f.read()
        except FileNotFoundError:
            print(f"{doc_id}: TEDS={teds_score:.3f} FILE NOT FOUND")
            continue
        
        gt_html = convert_to_markdown_with_html_tables(gt_md)
        pred_html = convert_to_markdown_with_html_tables(pred_md)
        gt_tables = extract_tables(gt_html)
        pred_tables = extract_tables(pred_html)
        
        gt_dims = [table_dims(t) for t in gt_tables]
        pred_dims = [table_dims(t) for t in pred_tables]
        
        # Categorize
        if len(pred_tables) == 0:
            cat = 'missing_pred'
        elif len(pred_tables) > len(gt_tables):
            cat = 'fragmented'
        elif teds_score >= 0.9:
            cat = 'good'
        elif teds_score >= 0.7:
            cat = 'close'
        else:
            # Check if total rows are more or less
            gt_total_rows = sum(r for r, c in gt_dims)
            pred_total_rows = sum(r for r, c in pred_dims)
            if pred_total_rows > gt_total_rows + 2:
                cat = 'extra_rows'
            else:
                cat = 'missing_rows_cols'
        
        categories[cat].append(doc_id)
        
        if teds_score < 0.9:
            print(f"{doc_id}: TEDS={teds_score:.3f} GT_tables={len(gt_tables)} {gt_dims} Pred_tables={len(pred_tables)} {pred_dims}")
    
    print()
    print("=== CATEGORIES ===")
    for cat, docs in categories.items():
        print(f"{cat}: {len(docs)} docs")
    
    print()
    print(f"Good (>=0.9): {len(categories['good'])}")
    print(f"Close (0.7-0.9): {len(categories['close'])}")
    print(f"Under 0.7: {len(teds_docs) - len(categories['good']) - len(categories['close'])}")
    
    # Show the improvement potential
    total_teds = sum(s for _, s in teds_docs)
    print(f"\nCurrent TEDS mean: {total_teds / len(teds_docs):.4f}")
    
    # If we could fix fragmented tables to 0.7 minimum
    improved_teds = total_teds
    for doc_id, score in teds_docs:
        if doc_id in categories['fragmented'] and score < 0.7:
            improved_teds += (0.7 - score)
    print(f"If fragmented -> 0.7: {improved_teds / len(teds_docs):.4f}")


if __name__ == '__main__':
    main()
