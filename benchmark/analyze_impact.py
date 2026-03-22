"""Analyze per-doc impact on Overall score."""
import os
import sys

sys.path.insert(0, "src")
from evaluator_table import evaluate_table
from evaluator_heading_level import evaluate_heading_level
from evaluator_reading_order import evaluate_reading_order

gt_dir = "ground-truth/markdown"
pred_dir = "prediction/edgeparse/markdown"

docs = []
for f in sorted(os.listdir(gt_dir)):
    if not f.endswith(".md"):
        continue
    doc_id = f.replace(".md", "")
    gt_f = os.path.join(gt_dir, f)
    pred_f = os.path.join(pred_dir, f)
    if not os.path.exists(pred_f):
        continue

    with open(gt_f) as g:
        gt = g.read()
    with open(pred_f) as p:
        pred = p.read()

    nid_result = evaluate_reading_order(gt, pred)
    nid = nid_result[0] if isinstance(nid_result, tuple) else nid_result
    teds_result = evaluate_table(gt, pred)
    teds = teds_result[0] if teds_result else None
    mhs_result = evaluate_heading_level(gt, pred)

    metrics = {"nid": nid}
    if teds is not None:
        metrics["teds"] = teds
    if mhs_result is not None and mhs_result[0] is not None:
        metrics["mhs"] = mhs_result[0]

    per_doc_avg = sum(metrics.values()) / len(metrics)
    docs.append({"id": doc_id, "metrics": metrics, "avg": per_doc_avg})

# Current overall
overall = sum(d["avg"] for d in docs) / len(docs)
print(f"Overall: {overall:.4f} (from {len(docs)} docs)")
print()

# Find docs with worst per-doc averages
docs_sorted = sorted(docs, key=lambda d: d["avg"])
print("Worst 25 per-doc averages:")
for d in docs_sorted[:25]:
    m = d["metrics"]
    parts = [f"nid={m['nid']:.3f}"]
    if "teds" in m:
        parts.append(f"teds={m['teds']:.3f}")
    if "mhs" in m:
        parts.append(f"mhs={m['mhs']:.3f}")
    n_metrics = len(m)
    print(f"  {d['id'][-3:]}: avg={d['avg']:.4f}  ({n_metrics} metrics)  {' '.join(parts)}")

# Show which metrics are missing for worst docs
print()
print("Metric availability for worst docs:")
for d in docs_sorted[:15]:
    has = list(d["metrics"].keys())
    missing = [m for m in ["nid", "teds", "mhs"] if m not in has]
    print(f"  {d['id'][-3:]}: has={has}, missing={missing}")

# Simulate improvements
print()
print("Simulated improvements (impact on Overall):")
target = 0.8823
gap = target - overall
print(f"Current gap to target: {gap:.4f}")
print()

# What if we improve the worst MHS docs?
mhs_docs = [(d, d["metrics"].get("mhs", None)) for d in docs if "mhs" in d["metrics"]]
mhs_docs_sorted = sorted(mhs_docs, key=lambda x: x[1])
print("If worst 5 MHS docs improved by +0.3:")
total_improvement = 0
for d, mhs_score in mhs_docs_sorted[:5]:
    n_metrics = len(d["metrics"])
    delta_overall = 0.3 / n_metrics / len(docs)
    total_improvement += delta_overall
    print(f"  {d['id'][-3:]}: MHS {mhs_score:.3f} -> {mhs_score+0.3:.3f}, delta_overall={delta_overall:.5f}")
print(f"  Total: +{total_improvement:.5f}")

# What if worst 5 TEDS improve by +0.3?
teds_docs = [(d, d["metrics"].get("teds", None)) for d in docs if "teds" in d["metrics"]]
teds_docs_sorted = sorted(teds_docs, key=lambda x: x[1])
print()
print("If worst 5 TEDS docs improved by +0.3:")
total_improvement = 0
for d, teds_score in teds_docs_sorted[:5]:
    n_metrics = len(d["metrics"])
    delta_overall = 0.3 / n_metrics / len(docs)
    total_improvement += delta_overall
    print(f"  {d['id'][-3:]}: TEDS {teds_score:.3f} -> {teds_score+0.3:.3f}, delta_overall={delta_overall:.5f}")
print(f"  Total: +{total_improvement:.5f}")
