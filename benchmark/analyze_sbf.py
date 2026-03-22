#!/usr/bin/env python3
"""Analyze SBF to understand paragraph boundary issues."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "src"))
from evaluator_paragraph import split_prose_blocks

benchmark_dir = Path(__file__).parent
gt_dir = benchmark_dir / "ground-truth" / "markdown"
pred_dir = benchmark_dir / "prediction" / "edgeparse" / "markdown"
eval_path = benchmark_dir / "prediction" / "edgeparse" / "evaluation.json"

with open(eval_path) as f:
    data = json.load(f)

over_merged = 0  # pred has fewer blocks than GT
under_merged = 0  # pred has more blocks than GT
exact = 0
total_gt = 0
total_pred = 0

for doc in data["documents"]:
    did = doc["document_id"]
    sbf = doc["scores"].get("prose_block_boundary_f1")
    if sbf is None:
        continue
    
    gt_file = gt_dir / f"{did}.md"
    pred_file = pred_dir / f"{did}.md"
    if not gt_file.exists() or not pred_file.exists():
        continue
    
    gt_blocks = split_prose_blocks(gt_file.read_text())
    pred_blocks = split_prose_blocks(pred_file.read_text())
    
    total_gt += len(gt_blocks)
    total_pred += len(pred_blocks)
    
    if len(pred_blocks) < len(gt_blocks):
        over_merged += 1
    elif len(pred_blocks) > len(gt_blocks):
        under_merged += 1
    else:
        exact += 1

print(f"Over-merged (fewer pred blocks): {over_merged}")
print(f"Under-merged (more pred blocks): {under_merged}")
print(f"Exact count match: {exact}")
print(f"Total GT blocks: {total_gt}, Total Pred blocks: {total_pred}")
print(f"Mean GT blocks/doc: {total_gt/200:.1f}, Mean Pred blocks/doc: {total_pred/200:.1f}")

# Show worst SBF docs with block counts
print("\nWorst SBF docs:")
worst = []
for doc in data["documents"]:
    sbf = doc["scores"].get("prose_block_boundary_f1")
    if sbf is not None and sbf < 0.5:
        worst.append((doc["document_id"], sbf, 
                      doc["scores"].get("gt_prose_block_count", 0),
                      doc["scores"].get("pred_prose_block_count", 0)))
worst.sort(key=lambda x: x[1])
for did, sbf, gt_c, pred_c in worst[:25]:
    gt_file = gt_dir / f"{did}.md"
    pred_file = pred_dir / f"{did}.md"
    gt_blocks = len(split_prose_blocks(gt_file.read_text())) if gt_file.exists() else 0
    pred_blocks = len(split_prose_blocks(pred_file.read_text())) if pred_file.exists() else 0
    direction = "OVER" if pred_blocks < gt_blocks else "UNDER" if pred_blocks > gt_blocks else "SAME"
    print(f"  {did}: SBF={sbf:.4f} GT={gt_blocks} Pred={pred_blocks} ({direction})")
