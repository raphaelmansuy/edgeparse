"""Compare GT and pred table content for docs 088, 089, 090 to find extra rows."""
import os

docs = ['01030000000088', '01030000000089', '01030000000090']
for doc_id in docs:
    print(f"\n{'='*60}")
    print(f"Doc {doc_id}")
    print(f"{'='*60}")
    
    gt_path = f'ground-truth/markdown/{doc_id}.md'
    pred_path = f'prediction/edgeparse/markdown/{doc_id}.md'
    
    with open(gt_path) as f:
        gt = f.read()
    with open(pred_path) as f:
        pred = f.read()
    
    # Extract pipe table rows
    def get_table_rows(text):
        rows = []
        for line in text.split('\n'):
            line = line.strip()
            if line.startswith('|') and line.endswith('|'):
                # Skip separator
                cells = [c.strip() for c in line.split('|')[1:-1]]
                if all(c.replace('-', '').replace(':', '').strip() == '' for c in cells):
                    continue
                rows.append(cells)
        return rows
    
    gt_rows = get_table_rows(gt)
    pred_rows = get_table_rows(pred)
    
    print(f"\nGT rows ({len(gt_rows)}):")
    for i, row in enumerate(gt_rows):
        print(f"  {i}: {row}")
    
    print(f"\nPred rows ({len(pred_rows)}):")
    for i, row in enumerate(pred_rows):
        print(f"  {i}: {row}")
