#!/usr/bin/env python3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / 'src'))
from converter_markdown_table import convert_to_markdown_with_html_tables
from bs4 import BeautifulSoup

docs = [132, 180, 146, 127, 89, 88, 200, 182, 122, 178]

for d in docs:
    did = f"01030000000{d:03d}"
    gt_path = Path(__file__).parent / 'ground-truth' / 'markdown' / f'{did}.md'
    pred_path = Path(__file__).parent / 'prediction' / 'edgeparse' / 'markdown' / f'{did}.md'
    
    gt = gt_path.read_text() if gt_path.exists() else ""
    pred = pred_path.read_text() if pred_path.exists() else ""
    
    gt_r = convert_to_markdown_with_html_tables(gt)
    pred_r = convert_to_markdown_with_html_tables(pred)
    
    gt_t = BeautifulSoup(gt_r, 'html.parser').find_all('table')
    pred_t = BeautifulSoup(pred_r, 'html.parser').find_all('table')
    
    gt_rows = sum(len(t.find_all('tr')) for t in gt_t)
    pred_rows = sum(len(t.find_all('tr')) for t in pred_t)
    
    def max_cols(tables):
        mc = 0
        for t in tables:
            for tr in t.find_all('tr'):
                c = len(tr.find_all(['th', 'td']))
                mc = max(mc, c)
        return mc
    
    print(f"doc {d:03d}: GT={len(gt_t)} tables/{gt_rows} rows/max {max_cols(gt_t)} cols  PRED={len(pred_t)} tables/{pred_rows} rows/max {max_cols(pred_t)} cols")
