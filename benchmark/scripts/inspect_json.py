#!/usr/bin/env python3
"""Inspect the JSON output for a document."""
import json
import sys

doc_id = sys.argv[1] if len(sys.argv) > 1 else "01030000000020"
json_path = f"/tmp/out{doc_id[-2:]}/{doc_id}.json"

with open(json_path) as f:
    d = json.load(f)

def print_tree(node, depth=0, limit=200):
    if depth > 5:
        return
    if isinstance(node, dict):
        t = node.get('type', '?')
        text = ''
        if 'content' in node:
            text = str(node['content'])[:80]
        elif 'text' in node and isinstance(node['text'], str):
            text = node['text'][:80]
        elif 'text_lines' in node:
            lines = node['text_lines']
            text = ' | '.join(l.get('text', '') for l in lines[:3])[:80]
        
        indent = '  ' * depth
        print(f"{indent}[{t}] {text!r}")
        
        for k, v in node.items():
            if k == 'kids' and isinstance(v, list):
                for child in v:
                    print_tree(child, depth + 1, limit)

print_tree(d)
