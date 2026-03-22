#!/usr/bin/env python3
"""Check for trailing page numbers/footers in edgeparse predictions."""
import re
from pathlib import Path

pred_dir = Path('prediction/edgeparse/markdown')
gt_dir = Path('ground-truth/markdown')

# Common page number patterns at end of document
page_patterns = [
    r'^\d+\s*\|.*$',           # "42 | Ch. 3. The Federal Tax System"
    r'^.*\|\s*\d+\s*$',        # "Ch. 3. | 42"
    r'^\d{1,4}\s*$',           # Just a number alone
    r'^Page\s+\d+',            # "Page 42"
    r'^\d+\s+of\s+\d+',       # "2 of 5"
]

found = 0
for pred_path in sorted(pred_dir.glob('*.md')):
    doc_id = pred_path.stem
    pred_md = pred_path.read_text().strip()
    if not pred_md:
        continue
    
    lines = pred_md.split('\n')
    # Check last 3 non-empty lines
    non_empty = [l.strip() for l in lines if l.strip()]
    if not non_empty:
        continue
    
    last_lines = non_empty[-3:]
    for line in last_lines:
        for pat in page_patterns:
            if re.match(pat, line):
                # Check if this text is in ground truth
                gt_path = gt_dir / pred_path.name
                gt_has = False
                if gt_path.exists():
                    gt_md = gt_path.read_text()
                    gt_has = line[:30] in gt_md
                if not gt_has:
                    found += 1
                    print(f'  {doc_id}: "{line[:80]}"')
                break

print(f'\nTotal docs with trailing page/footer artifacts: {found}')
