"""Check for missing/extra spaces in table cell text - word break issues."""
import os
import re

md_dir = 'prediction/edgeparse/markdown'

# Pattern: lowercase followed immediately by uppercase (like "orborders", "theacquisition")
missing_space_pattern = re.compile(r'[a-z][A-Z]')
# Pattern: words joined without space that should have one
# e.g., "containsor" — harder to detect without dictionary

issues = {}
for fname in sorted(os.listdir(md_dir)):
    if not fname.endswith('.md'):
        continue
    doc_id = fname.replace('.md', '')
    with open(os.path.join(md_dir, fname)) as f:
        pred = f.read()
    
    table_lines = [l for l in pred.split('\n') if l.strip().startswith('|') and l.strip().endswith('|')]
    if not table_lines:
        continue
    
    doc_issues = []
    for line in table_lines:
        cells = line.split('|')[1:-1]
        for cell in cells:
            cell = cell.strip()
            if len(cell) < 5:
                continue
            # Find lowercase-uppercase transitions (missing space)
            matches = list(missing_space_pattern.finditer(cell))
            for m in matches:
                # Skip common patterns like "McCann", "iPhone"
                word_ctx = cell[max(0,m.start()-10):m.end()+10]
                doc_issues.append(word_ctx)
    
    if doc_issues:
        # Only show docs with tables that have TEDS scores  
        issues[doc_id] = doc_issues

# Show top docs
for doc_id, doc_issues in sorted(issues.items()):
    if len(doc_issues) > 2:
        print(f"\n{doc_id} ({len(doc_issues)} camelCase joins):")
        for issue in doc_issues[:10]:
            print(f"  '{issue}'")
