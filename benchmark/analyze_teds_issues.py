"""Analyze TEDS issues for worst table docs."""
import os
import sys
import re

sys.path.insert(0, "src")
from evaluator_table import evaluate_table

gt_dir = "ground-truth/markdown"
pred_dir = "prediction/edgeparse/markdown"


def count_table_dims(text):
    """Extract table dimensions from markdown."""
    # Look for pipe tables
    pipe_lines = [l for l in text.split("\n") if "|" in l and l.strip().startswith("|")]
    if pipe_lines:
        # Count columns from first data row
        cols = max(len(l.split("|")) - 2 for l in pipe_lines) if pipe_lines else 0
        # Count rows (exclude separator)
        rows = len([l for l in pipe_lines if not re.match(r"^\s*\|[\s\-:|]+\|", l)])
        return rows, cols, "pipe"
    
    # Look for HTML tables
    if "<table" in text.lower():
        import html.parser
        row_count = text.lower().count("<tr")
        return row_count, 0, "html"
    
    return 0, 0, "none"


# Analyze worst TEDS docs
worst_docs = [
    "01030000000122",
    "01030000000178",
    "01030000000132",
    "01030000000180",
    "01030000000200",
    "01030000000182",
    "01030000000146",
    "01030000000127",
    "01030000000187",
    "01030000000188",
    "01030000000116",
    "01030000000170",
    "01030000000150",
    "01030000000119",
    "01030000000047",
]

for doc_id in worst_docs:
    gt_f = os.path.join(gt_dir, doc_id + ".md")
    pred_f = os.path.join(pred_dir, doc_id + ".md")
    
    if not os.path.exists(gt_f) or not os.path.exists(pred_f):
        continue
    
    with open(gt_f) as g:
        gt = g.read()
    with open(pred_f) as p:
        pred = p.read()
    
    result = evaluate_table(gt, pred)
    if result is None or result[0] is None:
        continue
    
    teds = result[0]
    teds_s = result[1] if len(result) > 1 else None
    
    gt_rows, gt_cols, gt_type = count_table_dims(gt)
    pred_rows, pred_cols, pred_type = count_table_dims(pred)
    
    # Count tables
    gt_tables = gt.lower().count("<table") + len(re.findall(r"^\|", gt, re.MULTILINE))
    pred_tables = pred.lower().count("<table") + len(re.findall(r"^\|", pred, re.MULTILINE))
    
    print(f"Doc {doc_id[-3:]}: TEDS={teds:.3f}" + (f" TEDS-S={teds_s:.3f}" if teds_s else ""))
    print(f"  GT:   {gt_type} table, ~{gt_rows} rows x {gt_cols} cols")
    print(f"  Pred: {pred_type} table, ~{pred_rows} rows x {pred_cols} cols")
    
    # Quick check for specific issues
    if gt_type == "none" and pred_type == "none":
        print(f"  ISSUE: No table detected in either GT or pred!")
    elif gt_type != "none" and pred_type == "none":
        print(f"  ISSUE: Table missing in prediction!")
    elif gt_rows != pred_rows:
        print(f"  ISSUE: Row count mismatch ({gt_rows} vs {pred_rows})")
    if gt_cols != pred_cols and gt_cols > 0 and pred_cols > 0:
        print(f"  ISSUE: Column count mismatch ({gt_cols} vs {pred_cols})")
    print()
