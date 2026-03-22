import json, pathlib, statistics

reports = sorted(pathlib.Path('reports').glob('benchmark-*.json'))
data = json.loads(reports[-1].read_text())
docs = data['documents']

overalls = [d['overall'] for d in docs if d.get('overall') is not None]
print(f'Overall: mean={statistics.mean(overalls):.4f}, n={len(overalls)}')

worst_overall = sorted(docs, key=lambda d: d.get('overall', 1))[:15]
print('\nWorst 15 overall:')
for d in worst_overall:
    nid = f"{d['nid']:.3f}" if d.get('nid') is not None else 'N/A'
    teds = f"{d['teds']:.3f}" if d.get('teds') is not None else 'N/A'
    mhs = f"{d['mhs']:.3f}" if d.get('mhs') is not None else 'N/A'
    print(f"  {d['document_id']}: overall={d['overall']:.3f} nid={nid} teds={teds} mhs={mhs}")

teds_docs = [(d['document_id'], d['teds']) for d in docs if d.get('teds') is not None]
teds_docs.sort(key=lambda x: x[1])
print(f'\nTEDS: mean={statistics.mean([t for _,t in teds_docs]):.4f}, n={len(teds_docs)}')
print('Worst 10 TEDS:')
for did, t in teds_docs[:10]:
    print(f'  {did}: {t:.3f}')

mhs_docs = [(d['document_id'], d['mhs']) for d in docs if d.get('mhs') is not None]
mhs_docs.sort(key=lambda x: x[1])
print(f'\nMHS: mean={statistics.mean([t for _,t in mhs_docs]):.4f}, n={len(mhs_docs)}')
print('Worst 15 MHS:')
for did, t in mhs_docs[:15]:
    print(f'  {did}: {t:.3f}')

nid_docs = [(d['document_id'], d['nid']) for d in docs if d.get('nid') is not None]
nid_docs.sort(key=lambda x: x[1])
print(f'\nNID: mean={statistics.mean([t for _,t in nid_docs]):.4f}, n={len(nid_docs)}')
print('Worst 15 NID:')
for did, t in nid_docs[:15]:
    print(f'  {did}: {t:.3f}')
