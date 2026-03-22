#!/usr/bin/env python3
"""Analyze gaps between edgeparse and docling scores per doc."""
import csv
import sys

def load_scores(path):
    scores = {}
    with open(path) as f:
        reader = csv.DictReader(f)
        for row in reader:
            doc_id = row['document_id'].lstrip("'")
            nid = float(row['nid']) if row['nid'] else None
            teds = float(row['teds']) if row['teds'] else None
            mhs = float(row['mhs']) if row['mhs'] else None
            metrics = [v for v in [nid, teds, mhs] if v is not None]
            overall = sum(metrics) / len(metrics) if metrics else 0.0
            scores[doc_id] = {'nid': nid, 'teds': teds, 'mhs': mhs, 'overall': overall}
    return scores

ep = load_scores('prediction/edgeparse/evaluation.csv')
doc = load_scores('prediction/docling/evaluation.csv')

# Calculate per-doc gaps (docling - edgeparse). Positive = docling better.
gaps = []
for doc_id in ep:
    if doc_id in doc:
        gap = doc[doc_id]['overall'] - ep[doc_id]['overall']
        gaps.append((doc_id, gap, ep[doc_id], doc[doc_id]))

# Sort by gap (docling advantage, largest first)
gaps.sort(key=lambda x: -x[1])

print("=== Docs where Docling beats us most (top 30) ===")
print(f"{'DocID':>15} {'Gap':>8} {'EP_ovr':>8} {'Doc_ovr':>8} {'EP_NID':>8} {'Doc_NID':>8} {'EP_TEDS':>8} {'Doc_TEDS':>8} {'EP_MHS':>8} {'Doc_MHS':>8}")
total_gap = 0
for doc_id, gap, ep_s, doc_s in gaps[:30]:
    total_gap += gap
    def fmt(v): return f"{v:.4f}" if v is not None else "  N/A  "
    print(f"{doc_id:>15} {gap:>+8.4f} {ep_s['overall']:>8.4f} {doc_s['overall']:>8.4f} {fmt(ep_s['nid']):>8} {fmt(doc_s['nid']):>8} {fmt(ep_s['teds']):>8} {fmt(doc_s['teds']):>8} {fmt(ep_s['mhs']):>8} {fmt(doc_s['mhs']):>8}")

print(f"\nTotal gap in top 30 docs: {total_gap:.4f} (= {total_gap/200:.4f} Overall impact)")

# NID-only docs (no TEDS, no MHS) where docling beats us
print("\n=== NID-only docs where Docling beats us ===")
nid_only_gaps = [(d, g, e, dc) for d, g, e, dc in gaps if e['teds'] is None and e['mhs'] is None and g > 0]
nid_only_gaps.sort(key=lambda x: -x[1])
for doc_id, gap, ep_s, doc_s in nid_only_gaps[:15]:
    print(f"  {doc_id}: EP_NID={ep_s['nid']:.4f} Doc_NID={doc_s['nid']:.4f} gap={gap:+.4f}")

# Summary statistics
total_gap_all = sum(g for _, g, _, _ in gaps)
doc_wins = sum(1 for _, g, _, _ in gaps if g > 0)
ep_wins = sum(1 for _, g, _, _ in gaps if g < 0)
print(f"\n=== Summary ===")
print(f"Total gap (docling-edgeparse): {total_gap_all:.4f} / 200 = {total_gap_all/200:.4f}")
print(f"Docling wins: {doc_wins}, Edgeparse wins: {ep_wins}")

# Metric-specific gaps
print("\n=== Per-metric gaps (where both have scores) ===")
for metric in ['nid', 'teds', 'mhs']:
    pairs = [(ep[d][metric], doc[d][metric]) for d in ep if d in doc and ep[d][metric] is not None and doc[d][metric] is not None]
    if pairs:
        ep_avg = sum(e for e, _ in pairs) / len(pairs)
        doc_avg = sum(d for _, d in pairs) / len(pairs)
        print(f"  {metric.upper()}: EP={ep_avg:.4f} Doc={doc_avg:.4f} gap={doc_avg-ep_avg:+.4f} (n={len(pairs)})")

# Docs where we beat docling most
print("\n=== Docs where we beat Docling most (top 15) ===")
gaps.sort(key=lambda x: x[1])
for doc_id, gap, ep_s, doc_s in gaps[:15]:
    def fmt(v): return f"{v:.4f}" if v is not None else "  N/A  "
    print(f"  {doc_id}: gap={gap:+.4f} EP={ep_s['overall']:.4f} Doc={doc_s['overall']:.4f}")
