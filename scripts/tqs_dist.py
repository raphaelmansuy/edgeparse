#!/usr/bin/env python3
"""Analyze TQS distribution and find systemic text quality issues."""
import json, os, glob

bench = os.path.join(os.path.dirname(__file__), '..', 'benchmark')
ev = json.load(open(os.path.join(bench, 'prediction/edgeparse/evaluation.json')))
docs = ev['documents']

tqs_all = [(d['scores'].get('text_quality_score'), d['document_id'],
            d['scores'].get('nid'), d['scores'].get('overall'))
           for d in docs if d['scores'].get('text_quality_score') is not None]
tqs_all.sort(key=lambda x: x[0])

print(f"Total docs with TQS: {len(tqs_all)}")
print(f"Average TQS: {sum(x[0] for x in tqs_all)/len(tqs_all):.4f}")
print()
print("20 lowest TQS docs:")

def fmt(value):
    return f"{value:.3f}" if value is not None else "N/A"

for tqs, did, nid, ov in tqs_all[:20]:
    print(f"  {did}: tqs={fmt(tqs)}  nid={fmt(nid)}  overall={fmt(ov)}")
