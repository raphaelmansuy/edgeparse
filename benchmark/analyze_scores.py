import json
import sys

with open('reports/benchmark-20260322-173226.json') as f:
    data = json.load(f)

for engine in data['engines']:
    if engine['name'] == 'edgeparse':
        docs = engine['documents']
        
        print('=== WORST TEDS DOCS ===')
        teds_docs = [(d['id'], d.get('teds', -1)) for d in docs if isinstance(d.get('teds'), (int, float))]
        teds_docs.sort(key=lambda x: x[1])
        for doc_id, score in teds_docs[:15]:
            print(f'  {doc_id}: {score:.4f}')
        
        print()
        print('=== WORST MHS DOCS ===')
        mhs_docs = [(d['id'], d.get('mhs', -1)) for d in docs if isinstance(d.get('mhs'), (int, float))]
        mhs_docs.sort(key=lambda x: x[1])
        for doc_id, score in mhs_docs[:15]:
            print(f'  {doc_id}: {score:.4f}')
        
        print()
        print('=== WORST PBF DOCS ===')
        pbf_docs = [(d['id'], d.get('pbf', -1)) for d in docs if isinstance(d.get('pbf'), (int, float))]
        pbf_docs.sort(key=lambda x: x[1])
        for doc_id, score in pbf_docs[:15]:
            print(f'  {doc_id}: {score:.4f}')
        
        print()
        print('=== WORST NID DOCS ===')
        nid_docs = [(d['id'], d.get('nid', -1)) for d in docs if isinstance(d.get('nid'), (int, float))]
        nid_docs.sort(key=lambda x: x[1])
        for doc_id, score in nid_docs[:15]:
            print(f'  {doc_id}: {score:.4f}')
        
        # Summary stats
        print()
        print(f'Total docs: {len(docs)}')
        print(f'Docs with TEDS: {len(teds_docs)}')
        print(f'Docs with MHS: {len(mhs_docs)}')
        print(f'Docs with TEDS < 0.5: {sum(1 for _, s in teds_docs if s < 0.5)}')
        print(f'Docs with MHS == 0.0: {sum(1 for _, s in mhs_docs if s == 0.0)}')
        print(f'Docs with MHS < 0.5: {sum(1 for _, s in mhs_docs if s < 0.5)}')
        break
