#!/usr/bin/env python3
"""Assess EdgeParse native vs WASM quality on a single PDF.

Shared metrics module with thin native/WASM runners (DRY).
Usage:
  python assess_pdf.py /path/to/doc.pdf [--native-bin PATH] [--wasm-pkg PATH]
  python assess_pdf.py /path/to/doc.pdf --baseline /tmp/baseline.json
"""

from __future__ import annotations

import argparse
import difflib
import json
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

REPO_ROOT = Path(__file__).resolve().parents[2]


def _norm(s: str) -> str:
    return re.sub(r"\s+", "", s)


def _strip_browser_chrome(text: str) -> str:
    lines = []
    for line in text.splitlines():
        if "Unslopping AI | RAM" in line:
            continue
        if "facebookresearch.github.io" in line:
            continue
        if re.fullmatch(r"\d+/\d+", line.strip()):
            continue
        if re.match(r"^\d{1,2}/\d{1,2}/\d{2,4}", line.strip()):
            continue
        lines.append(line)
    return "\n".join(lines)


def pdftotext_pages(pdf: Path) -> dict[int, str]:
    info = subprocess.run(
        ["pdfinfo", str(pdf)], capture_output=True, text=True, check=False
    )
    pages = 1
    for line in info.stdout.splitlines():
        if line.startswith("Pages:"):
            pages = int(line.split(":", 1)[1].strip())
            break
    out: dict[int, str] = {}
    for p in range(1, pages + 1):
        r = subprocess.run(
            ["pdftotext", "-f", str(p), "-l", str(p), str(pdf), "-"],
            capture_output=True,
            text=True,
            check=False,
        )
        out[p] = _strip_browser_chrome(r.stdout)
    return out


def char_overlap(ref: str, hyp: str) -> tuple[float, float]:
    r, h = _norm(ref), _norm(hyp)
    if not r and not h:
        return 1.0, 1.0
    if not r:
        return 0.0, 0.0
    if not h:
        return 0.0, 1.0
    sm = difflib.SequenceMatcher(None, r, h, autojunk=False)
    matched = sum(b.size for b in sm.get_matching_blocks())
    return matched / len(r), matched / len(h)


def count_stray_w(text: str) -> int:
    return len(re.findall(r"\bW (?=[a-z])|\bW\n", text))


def title_present(text: str, needles: list[str] | None = None) -> bool:
    needles = needles or ["Towards RL", "Unslopping AI"]
    n = _norm(text)
    return any(_norm(needle) in n for needle in needles)


def swallowing_headers(json_path: Path | None) -> int:
    if json_path is None or not json_path.exists():
        return -1
    try:
        data = json.loads(json_path.read_text())
    except json.JSONDecodeError:
        return -1
    kids = data.get("kids") or []
    count = 0
    for elem in kids:
        if not isinstance(elem, dict):
            continue
        # Legacy schema
        if elem.get("type") == "header":
            bbox = elem.get("bounding box") or [0, 0, 0, 0]
            height = abs(bbox[3] - bbox[1]) if len(bbox) >= 4 else 0
            content = elem.get("content") or ""
            if height > 792 * 0.08 and not str(content).strip():
                count += 1
            continue
        # Internal schema
        hf = elem.get("HeaderFooter")
        if isinstance(hf, dict):
            bbox = hf.get("bbox") or {}
            height = abs(float(bbox.get("top_y", 0)) - float(bbox.get("bottom_y", 0)))
            contents = hf.get("contents") or []
            empty = not contents or all(
                not (c.get("content") or c.get("value") or "")
                for c in contents
                if isinstance(c, dict)
            )
            if height > 792 * 0.08 and empty:
                count += 1
    return count


def score_markdown(
    md: str,
    ref_pages: dict[int, str],
    *,
    json_path: Path | None = None,
    wall_ms: float,
    engine: str,
) -> dict[str, Any]:
    full_ref = "\n".join(ref_pages[p] for p in sorted(ref_pages))
    recall, prec = char_overlap(full_ref, md)
    ref_words = len(full_ref.split())
    out_words = len(md.split())
    page_scores = []
    # Approximate page split via form-feed if present; else single aggregate.
    parts = [p for p in md.split("\f") if p.strip()]
    if len(parts) != len(ref_pages):
        parts = [md]
    for i, pnum in enumerate(sorted(ref_pages)):
        page_md = parts[i] if i < len(parts) else md
        pr, pp = char_overlap(ref_pages[pnum], page_md)
        page_scores.append(
            {
                "page": pnum,
                "char_recall": round(pr, 4),
                "char_precision": round(pp, 4),
                "ref_words": len(ref_pages[pnum].split()),
                "out_words": len(page_md.split()),
            }
        )
    return {
        "engine": engine,
        "wall_ms": round(wall_ms, 1),
        "char_recall": round(recall, 4),
        "char_precision": round(prec, 4),
        "ref_words": ref_words,
        "out_words": out_words,
        "word_ratio": round(out_words / max(1, ref_words), 4),
        "stray_w": count_stray_w(md),
        "title_present": title_present(md),
        "swallowing_headers": swallowing_headers(json_path),
        "pages": page_scores,
    }


def run_native(pdf: Path, binary: Path, out_dir: Path) -> tuple[str, Path | None, float]:
    out_dir.mkdir(parents=True, exist_ok=True)
    t0 = time.perf_counter()
    env = dict(**__import__("os").environ)
    env["EDGEPARSE_RASTER_TABLE_OCR"] = env.get("EDGEPARSE_RASTER_TABLE_OCR", "off")
    subprocess.run(
        [
            str(binary),
            str(pdf),
            "-o",
            str(out_dir),
            "-f",
            "markdown,json",
            "--image-output",
            "off",
            "-q",
        ],
        check=True,
        env=env,
        capture_output=True,
    )
    wall = (time.perf_counter() - t0) * 1000
    stem = pdf.stem
    md = (out_dir / f"{stem}.md").read_text(errors="replace")
    jp = out_dir / f"{stem}.json"
    return md, jp if jp.exists() else None, wall


def run_wasm(pdf: Path, pkg: Path, out_dir: Path) -> tuple[str, Path | None, float]:
    out_dir.mkdir(parents=True, exist_ok=True)
    script = out_dir / "run_wasm.cjs"
    script.write_text(
        f"""
const fs = require('fs');
const w = require({json.dumps(str(pkg / 'edgeparse_wasm.js'))});
const pdf = fs.readFileSync({json.dumps(str(pdf))});
const t0 = Date.now();
const md = w.convert_to_string(new Uint8Array(pdf), 'markdown', undefined, undefined, undefined);
const js = w.convert_to_string(new Uint8Array(pdf), 'json', undefined, undefined, undefined);
fs.writeFileSync({json.dumps(str(out_dir / 'doc.md'))}, md);
fs.writeFileSync({json.dumps(str(out_dir / 'doc.json'))}, js);
console.log(JSON.stringify({{wall_ms: Date.now()-t0, version: w.version()}}));
"""
    )
    t0 = time.perf_counter()
    meta = subprocess.run(
        ["node", str(script)], capture_output=True, text=True, check=True
    )
    wall = (time.perf_counter() - t0) * 1000
    try:
        wall = float(json.loads(meta.stdout.strip().splitlines()[-1])["wall_ms"])
    except (json.JSONDecodeError, KeyError, IndexError, ValueError):
        pass
    md = (out_dir / "doc.md").read_text(errors="replace")
    jp = out_dir / "doc.json"
    return md, jp if jp.exists() else None, wall


def assess(
    pdf: Path,
    *,
    native_bin: Path | None,
    wasm_pkg: Path | None,
    work: Path,
) -> dict[str, Any]:
    ref_pages = pdftotext_pages(pdf)
    result: dict[str, Any] = {
        "pdf": str(pdf),
        "pages": len(ref_pages),
        "engines": {},
    }
    if native_bin and native_bin.exists():
        md, jp, wall = run_native(pdf, native_bin, work / "native")
        result["engines"]["native"] = score_markdown(
            md, ref_pages, json_path=jp, wall_ms=wall, engine="native"
        )
    if wasm_pkg and (wasm_pkg / "edgeparse_wasm.js").exists():
        md, jp, wall = run_wasm(pdf, wasm_pkg, work / "wasm")
        result["engines"]["wasm"] = score_markdown(
            md, ref_pages, json_path=jp, wall_ms=wall, engine="wasm"
        )
        if "native" in result["engines"]:
            nat = (work / "native" / f"{pdf.stem}.md").read_text(errors="replace")
            result["native_wasm_md_identical"] = nat == md
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("pdf", type=Path)
    ap.add_argument(
        "--native-bin",
        type=Path,
        default=REPO_ROOT / "target" / "release" / "edgeparse",
    )
    ap.add_argument(
        "--wasm-pkg",
        type=Path,
        default=REPO_ROOT / "crates" / "edgeparse-wasm" / "pkg-node",
    )
    ap.add_argument("--baseline", type=Path, help="Write/compare baseline JSON")
    ap.add_argument("--out", type=Path, help="Write assessment JSON")
    ap.add_argument("--skip-wasm", action="store_true")
    args = ap.parse_args()

    with tempfile.TemporaryDirectory(prefix="ep_assess_") as tmp:
        result = assess(
            args.pdf,
            native_bin=args.native_bin,
            wasm_pkg=None if args.skip_wasm else args.wasm_pkg,
            work=Path(tmp),
        )

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=2))
    if args.baseline:
        args.baseline.parent.mkdir(parents=True, exist_ok=True)
        if not args.baseline.exists():
            args.baseline.write_text(json.dumps(result, indent=2))
            print(f"Wrote baseline → {args.baseline}")
        else:
            prev = json.loads(args.baseline.read_text())
            print("Baseline comparison (word_ratio / recall / stray_w):")
            for eng in sorted(result["engines"]):
                a = prev.get("engines", {}).get(eng, {})
                b = result["engines"][eng]
                print(
                    f"  {eng}: {a.get('word_ratio')}→{b['word_ratio']}  "
                    f"recall {a.get('char_recall')}→{b['char_recall']}  "
                    f"stray_w {a.get('stray_w')}→{b['stray_w']}  "
                    f"ms {a.get('wall_ms')}→{b['wall_ms']}"
                )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
