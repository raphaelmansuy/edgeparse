#!/usr/bin/env python3
"""Explore edgeparse JSON output for a given doc."""
import json, sys

doc_id = sys.argv[1] if len(sys.argv) > 1 else '01030000000199'
path = f'/tmp/edgeparse_debug/{doc_id}.json'

with open(path) as f:
    data = json.load(f)

def explore(data, depth=0):
    prefix = '  ' * depth
    if isinstance(data, dict):
        if 'pages' in data:
            for pi, page in enumerate(data['pages']):
                print(f'{prefix}Page {pi}:')
                if 'elements' in page:
                    for i, e in enumerate(page['elements']):
                        show_element(e, i, depth+1)
        elif 'kids' in data:
            for i, k in enumerate(data['kids']):
                show_element(k, i, depth)
        elif 'elements' in data:
            for i, e in enumerate(data['elements']):
                show_element(e, i, depth)
        else:
            print(f'{prefix}Keys: {list(data.keys())[:10]}')
    elif isinstance(data, list):
        for i, item in enumerate(data):
            show_element(item, i, depth)

def show_element(e, idx, depth=0):
    prefix = '  ' * depth
    if isinstance(e, dict):
        etype = e.get('type', e.get('kind', e.get('category', '?')))
        text = ''
        if 'text' in e:
            text = str(e['text'])[:80]
        elif 'value' in e:
            text = str(e['value'])[:80]
        elif 'content' in e and isinstance(e['content'], dict):
            text = str(e['content'].get('text', ''))[:80]
        fs = e.get('font_size', e.get('fontSize', ''))
        mfs = e.get('max_font_size', e.get('maxFontSize', ''))
        fw = e.get('font_weight', '')
        fn = e.get('font_name', '')
        extra = ''
        if fs: extra += f' fs={fs}'
        if mfs: extra += f' mfs={mfs}'
        if fw: extra += f' fw={fw}'
        if fn: extra += f' fn={fn}'
        print(f'{prefix}[{idx}] {etype}{extra}: {text}')
    else:
        print(f'{prefix}[{idx}] {type(e).__name__}: {str(e)[:60]}')

explore(data)
