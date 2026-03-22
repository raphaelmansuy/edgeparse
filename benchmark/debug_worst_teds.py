"""Debug specific TEDS docs to find structural issues."""
import os

docs = [
    # (doc_id, description)
    ('01030000000122', 'missing pred tables'),
    ('01030000000132', '(5,2)->(1,1) truncated'),
    ('01030000000180', '(3,4)->(1,2) truncated'),
    ('01030000000182', '(4,4)->(4,3) missing col'),
    ('01030000000187', '(6,7)->(3,7) half rows'),
]

for doc_id, desc in docs:
    print(f"\n{'='*60}")
    print(f"Doc {doc_id}: {desc}")
    print(f"{'='*60}")
    
    gt_path = f'ground-truth/markdown/{doc_id}.md'
    pred_path = f'prediction/edgeparse/markdown/{doc_id}.md'
    
    with open(gt_path) as f:
        gt = f.read()
    
    if not os.path.exists(pred_path):
        print("  NO PREDICTION FILE")
        continue
    
    with open(pred_path) as f:
        pred = f.read()
    
    print(f"\nGT tables (looking for <table> or |...|):")
    gt_lines = gt.split('\n')
    for i, line in enumerate(gt_lines):
        if '<table>' in line.lower() or '|' in line:
            print(f"  L{i+1}: {line[:80]}")
    
    print(f"\nPred tables:")
    pred_lines = pred.split('\n')
    for i, line in enumerate(pred_lines):
        if line.strip().startswith('|') and line.strip().endswith('|'):
            print(f"  L{i+1}: {line[:100]}")
    
    # Show all text in pred
    print(f"\nPred full text (first 30 lines):")
    for i, line in enumerate(pred_lines[:30]):
        if line.strip():
            print(f"  L{i+1}: {line[:100]}")
