#!/usr/bin/env python3
"""Analyze GT heading text lengths to determine optimal MAX_HEADING_TEXT_LENGTH."""
import json

with open("ground-truth/reference.json") as f:
    data = json.load(f)

# Collect all categories
cats = set()
for doc_key, doc in data.items():
    for el in doc.get("elements", []):
        cats.add(el.get("category", ""))
print("All categories:", sorted(cats))

# Collect heading text lengths
lengths = []
for doc_key, doc in data.items():
    for el in doc.get("elements", []):
        cat = el.get("category", "")
        if "Heading" in cat or cat == "Title":
            text = el.get("content", {}).get("text", "")
            if text:
                lengths.append((len(text), text[:120], doc_key))

lengths.sort(key=lambda x: x[0], reverse=True)
print(f"\nTotal GT headings: {len(lengths)}")

if lengths:
    print(f"Max length: {lengths[0][0]}")
    p95 = lengths[int(len(lengths) * 0.05)]
    p90 = lengths[int(len(lengths) * 0.10)]
    p80 = lengths[int(len(lengths) * 0.20)]
    print(f"95th percentile: {p95[0]}")
    print(f"90th percentile: {p90[0]}")
    print(f"80th percentile: {p80[0]}")

    print("\nHeadings >= 70 chars:")
    for l, t, d in lengths:
        if l >= 70:
            print(f"  {d}: \"{t}\" ({l} chars)")
        else:
            break

    # Also count how many would be lost at various thresholds
    for threshold in [80, 90, 100, 120, 130]:
        lost = sum(1 for l, _, _ in lengths if l > threshold)
        print(f"\n  Headings > {threshold} chars: {lost} ({lost / len(lengths) * 100:.1f}%)")
