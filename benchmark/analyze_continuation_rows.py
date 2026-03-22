"""Find docs where table rows might be continuation rows (empty first cell)."""
import os

md_dir = 'prediction/edgeparse/markdown'

count = 0
for fname in sorted(os.listdir(md_dir)):
    if not fname.endswith('.md'):
        continue
    doc_id = fname.replace('.md', '')
    with open(os.path.join(md_dir, fname)) as f:
        pred = f.read()
    
    # Find pipe table rows (skip separators)
    table_rows = []
    for line in pred.split('\n'):
        line = line.strip()
        if not line.startswith('|') or not line.endswith('|'):
            continue
        cells = [c.strip() for c in line.split('|')[1:-1]]
        if all(c.replace('-', '').replace(':', '').strip() == '' for c in cells):
            continue
        table_rows.append(cells)
    
    if len(table_rows) < 2:
        continue
    
    # Check for continuation rows (first cell empty, at least one cell non-empty)
    continuation_rows = []
    for i in range(1, len(table_rows)):
        if not table_rows[i][0].strip():  # First cell empty
            has_content = any(c.strip() for c in table_rows[i])
            if has_content:
                continuation_rows.append(i)
    
    if continuation_rows:
        count += 1
        print(f"{doc_id}: {len(continuation_rows)} continuation rows out of {len(table_rows)} total")
        for ci in continuation_rows[:3]:
            print(f"  Row {ci}: {[c[:30] for c in table_rows[ci]]}")

print(f"\nTotal docs with continuation rows: {count}")
