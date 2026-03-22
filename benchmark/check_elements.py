"""Check element types in JSON output."""
import json
import sys

doc_id = sys.argv[1] if len(sys.argv) > 1 else "200"
fn = f"/tmp/edgeparse_debug/01030000000{doc_id}.json"

with open(fn) as f:
    data = json.load(f)

kids = data.get("kids", [])
print(f"Doc {doc_id}: {len(kids)} elements")
heading_count = 0
for i, kid in enumerate(kids):
    t = kid.get("type", "?")
    text = ""
    for key in ["text", "value", "content"]:
        if key in kid and isinstance(kid[key], str):
            text = kid[key][:80]
            break
    if t in ("heading", "number_heading"):
        heading_count += 1
        level = kid.get("level", "?")
        print(f"  {i:3d} [{t} L{level}] {text}")
    elif text and len(text.strip()) > 0 and len(text.strip()) < 80:
        print(f"  {i:3d} [{t:20s}] {text}")

print(f"\nPipeline heading count: {heading_count}")
