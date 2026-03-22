"""Analyze Unicode character differences between GT and predicted tables."""
import os
import re
import unicodedata

md_dir = 'prediction/edgeparse/markdown'
gt_dir = 'ground-truth/markdown'

# Unicode replacements that could help
UNICODE_ASCII_MAP = {
    '\u223c': '~',   # ∼ → ~
    '\u2212': '-',   # − → -
    '\u2013': '-',   # – → -
    '\u2014': '-',   # — → -
    '\u2018': "'",   # ' → '
    '\u2019': "'",   # ' → '
    '\u201c': '"',   # " → "
    '\u201d': '"',   # " → "
    '\u00d7': 'x',   # × → x
    '\u2264': '<=',  # ≤
    '\u2265': '>=',  # ≥
    '\u2260': '!=',  # ≠
    '\ufb01': 'fi',  # ﬁ → fi
    '\ufb02': 'fl',  # ﬂ → fl
    '\ufb03': 'ffi', # ﬃ → ffi
    '\ufb04': 'ffl', # ﬄ → ffl
    '\u00a0': ' ',   # non-breaking space
}

# Check which docs have these characters in predicted tables
docs_with_issues = {}
for fname in sorted(os.listdir(md_dir)):
    if not fname.endswith('.md'):
        continue
    doc_id = fname.replace('.md', '')
    with open(os.path.join(md_dir, fname)) as f:
        pred = f.read()
    
    # Find pipe table rows
    table_lines = [l for l in pred.split('\n') if l.strip().startswith('|') and l.strip().endswith('|')]
    if not table_lines:
        continue
    
    table_text = '\n'.join(table_lines)
    issues = {}
    for uchar, replacement in UNICODE_ASCII_MAP.items():
        count = table_text.count(uchar)
        if count > 0:
            issues[f"U+{ord(uchar):04X} ({unicodedata.name(uchar, '?')})"] = count
    
    # Also check GT for same chars
    gt_path = os.path.join(gt_dir, fname)
    if os.path.exists(gt_path):
        with open(gt_path) as f:
            gt = f.read()
        gt_table_lines = [l for l in gt.split('\n') if l.strip().startswith('|') and l.strip().endswith('|')]
        gt_text = '\n'.join(gt_table_lines)
        gt_issues = {}
        for uchar, replacement in UNICODE_ASCII_MAP.items():
            gt_count = gt_text.count(uchar)
            if gt_count > 0:
                gt_issues[f"U+{ord(uchar):04X}"] = gt_count
    else:
        gt_issues = {}
    
    if issues:
        docs_with_issues[doc_id] = (issues, gt_issues)

print(f"Docs with Unicode issues in tables: {len(docs_with_issues)}\n")
for doc_id, (pred_issues, gt_issues) in sorted(docs_with_issues.items()):
    print(f"  {doc_id}:")
    for char_desc, count in pred_issues.items():
        gt_has = any(char_desc.split(' ')[0] in k for k in gt_issues)
        print(f"    Pred: {char_desc} x{count}  {'(GT has same)' if gt_has else '(GT uses ASCII)'}")
