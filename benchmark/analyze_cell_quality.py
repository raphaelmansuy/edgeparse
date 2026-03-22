"""Analyze cell content quality in predicted tables - look for letter-spacing issues."""
import os
import re
import sys

md_dir = 'prediction/edgeparse/markdown'
gt_dir = 'ground-truth/markdown'

# Check a few docs with bad TEDS
docs = ['01030000000089', '01030000000088', '01030000000090', '01030000000132',
        '01030000000180', '01030000000182', '01030000000127', '01030000000187',
        '01030000000119', '01030000000188', '01030000000047', '01030000000046']

for doc_id in docs:
    pred_path = os.path.join(md_dir, f'{doc_id}.md')
    gt_path = os.path.join(gt_dir, f'{doc_id}.md')
    if not os.path.exists(pred_path):
        continue
    
    with open(pred_path) as f:
        pred = f.read()
    
    # Find pipe table rows
    pipe_rows = [l for l in pred.split('\n') if l.strip().startswith('|') and l.strip().endswith('|')]
    if not pipe_rows:
        continue
    
    # Check for letter-spacing (single chars separated by spaces in cells)
    letter_spaced = []
    for row in pipe_rows:
        cells = row.split('|')[1:-1]  # Skip outer empty from split
        for cell in cells:
            cell = cell.strip()
            if not cell:
                continue
            # Letter-spaced pattern: mostly single chars separated by spaces
            tokens = cell.split()
            if len(tokens) >= 3:
                single_chars = sum(1 for t in tokens if len(t) == 1)
                if single_chars >= len(tokens) * 0.6:
                    letter_spaced.append(cell)
    
    # Check for fragmented words (short fragments)
    fragmented = []
    for row in pipe_rows:
        cells = row.split('|')[1:-1]
        for cell in cells:
            cell = cell.strip()
            if not cell:
                continue
            tokens = cell.split()
            if len(tokens) >= 2:
                short = sum(1 for t in tokens if 1 < len(t) <= 3 and t.isalpha())
                if short >= 2 and short >= len(tokens) * 0.4:
                    fragmented.append(cell)
    
    if letter_spaced or fragmented:
        print(f"\n=== Doc {doc_id} ===")
        if letter_spaced:
            print(f"  Letter-spaced ({len(letter_spaced)}):")
            for ls in letter_spaced[:5]:
                print(f"    '{ls}'")
        if fragmented:
            print(f"  Fragmented ({len(fragmented)}):")
            for fg in fragmented[:5]:
                print(f"    '{fg}'")
