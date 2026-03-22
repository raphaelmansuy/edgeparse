"""Analyze worst TEDS docs to find patterns."""
import sys
sys.path.insert(0, 'src')

from pathlib import Path
from evaluator_table import evaluate_table, extract_tables
from converter_markdown_table import convert_to_markdown_with_html_tables

GT_DIR = Path("ground-truth/markdown")
PRED_DIR = Path("prediction/edgeparse/markdown")

worst_docs = [
    "01030000000122",
    "01030000000178", 
    "01030000000132",
    "01030000000180",
    "01030000000200",
    "01030000000182",
    "01030000000146",
    "01030000000127",
    "01030000000089",
    "01030000000088",
]

for doc_id in worst_docs:
    gt_md = (GT_DIR / f"{doc_id}.md").read_text(encoding="utf-8")
    pred_path = PRED_DIR / f"{doc_id}.md"
    pred_md = pred_path.read_text(encoding="utf-8") if pred_path.exists() else ""
    
    gt_html = convert_to_markdown_with_html_tables(gt_md)
    pred_html = convert_to_markdown_with_html_tables(pred_md)
    
    gt_tables = extract_tables(gt_html)
    pred_tables = extract_tables(pred_html)
    
    teds, teds_s = evaluate_table(gt_md, pred_md)
    
    # Count rows/cols in GT tables
    gt_info = []
    for t in gt_tables:
        rows = t.count("<tr")
        gt_info.append(f"{rows}rows")
    
    pred_info = []
    for t in pred_tables:
        rows = t.count("<tr")
        pred_info.append(f"{rows}rows")
    
    # Check if prediction has pipe tables
    pipe_count = pred_md.count("| ")
    
    print(f"{doc_id}: TEDS={teds:.4f} GT_tables={len(gt_tables)} ({', '.join(gt_info)}) Pred_tables={len(pred_tables)} ({', '.join(pred_info)}) pipes={pipe_count}")
