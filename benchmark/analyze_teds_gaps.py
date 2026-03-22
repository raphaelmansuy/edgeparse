#!/usr/bin/env python3
"""Analyze TEDS gaps between edgeparse and docling."""
import csv

def load_scores(path):
    scores = {}
    with open(path) as f:
        reader = csv.DictReader(f)
        for row in reader:
            doc_id = row['document_id'].lstrip("'")
            teds = float(row['teds']) if row['teds'] else None
            teds_s = float(row['teds_s']) if row['teds_s'] else None
            scores[doc_id] = {'teds': teds, 'teds_s': teds_s}
    return scores

ep = load_scores('prediction/edgeparse/evaluation.csv')
doc = load_scores('prediction/docling/evaluation.csv')

print("=== TEDS comparison: Edgeparse vs Docling ===")
print(f"{'DocID':>15} {'EP_TEDS':>8} {'Doc_TEDS':>8} {'Gap':>8} {'EP_TEDSS':>8} {'Doc_TEDSS':>8}")

teds_docs = []
for d in ep:
    if ep[d]['teds'] is not None and d in doc and doc[d]['teds'] is not None:
        gap = doc[d]['teds'] - ep[d]['teds']
        teds_docs.append((d, ep[d]['teds'], doc[d]['teds'], gap, ep[d]['teds_s'], doc[d]['teds_s']))

teds_docs.sort(key=lambda x: -x[3])  # Sort by gap, docling advantage first

for d, ep_t, doc_t, gap, ep_ts, doc_ts in teds_docs:
    def fmt(v): return f"{v:.4f}" if v is not None else "  N/A  "
    print(f"{d:>15} {ep_t:>8.4f} {doc_t:>8.4f} {gap:>+8.4f} {fmt(ep_ts):>8} {fmt(doc_ts):>8}")

# Summary
ep_avg = sum(e for _, e, _, _, _, _ in teds_docs) / len(teds_docs)
doc_avg = sum(d for _, _, d, _, _, _ in teds_docs) / len(teds_docs)
print(f"\nAvg TEDS: EP={ep_avg:.4f} Doc={doc_avg:.4f} Gap={doc_avg-ep_avg:+.4f}")
print(f"Docs with TEDS: {len(teds_docs)}")

# Categorize docs by gap severity
severe = [(d, e, dc, g) for d, e, dc, g, _, _ in teds_docs if g > 0.3]
moderate = [(d, e, dc, g) for d, e, dc, g, _, _ in teds_docs if 0.1 < g <= 0.3]
mild = [(d, e, dc, g) for d, e, dc, g, _, _ in teds_docs if 0 < g <= 0.1]
we_win = [(d, e, dc, g) for d, e, dc, g, _, _ in teds_docs if g <= 0]

print(f"\nSevere gap (>0.3): {len(severe)} docs")
print(f"Moderate gap (0.1-0.3): {len(moderate)} docs")
print(f"Mild gap (0-0.1): {len(mild)} docs")
print(f"We win or tie: {len(we_win)} docs")
