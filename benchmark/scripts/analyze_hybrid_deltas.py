import csv
import math


def load(path: str) -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = {}
    with open(path, newline="") as f:
        for row in csv.DictReader(f):
            doc = row["document_id"].strip().strip("'")
            out[doc] = row
    return out


def val(row: dict[str, str], key: str) -> float:
    raw = row.get(key, "").strip()
    if raw in ("", "none", "None"):
        return float("nan")
    return float(raw)


base = load("benchmark/prediction/edgeparse/evaluation.csv")
hybrid = load("benchmark/prediction/edgeparse_hybrid/evaluation.csv")

deltas = []
for doc in sorted(set(base).intersection(hybrid)):
    bo = val(base[doc], "overall")
    ho = val(hybrid[doc], "overall")
    if math.isnan(bo) or math.isnan(ho):
        continue
    deltas.append(
        (
            ho - bo,
            doc,
            bo,
            ho,
            val(base[doc], "teds"),
            val(hybrid[doc], "teds"),
            val(base[doc], "mhs"),
            val(hybrid[doc], "mhs"),
            val(base[doc], "token_boundary_f1"),
            val(hybrid[doc], "token_boundary_f1"),
        )
    )

deltas.sort(key=lambda x: x[0])
print("Top overall regressions (hybrid - baseline):")
for row in deltas[:20]:
    d, doc, bo, ho, tb, th, mb, mh, bb, bh = row
    print(
        f"{d:+.4f} doc={doc} base={bo:.4f} hyb={ho:.4f} "
        f"teds={tb:.3f}->{th:.3f} mhs={mb:.3f}->{mh:.3f} tbf1={bb:.3f}->{bh:.3f}"
    )

print("\nTop gains:")
for row in reversed(deltas[-20:]):
    d, doc, bo, ho, *_ = row
    print(f"{d:+.4f} doc={doc} base={bo:.4f} hyb={ho:.4f}")
