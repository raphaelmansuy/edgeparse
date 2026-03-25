#!/usr/bin/env python3
"""
Run pdf2md on all PDFs in benchmark/pdfs/ and save .md files to
benchmark/ground-truth/markdown/.
Idempotent: skips PDFs that already have a .md file unless --force.

Usage (from any directory):
    python3 /path/to/benchmark/scripts/gen_gt_markdown.py [--force]
"""
import argparse
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent  # benchmark/
PDF_DIR = HERE / "pdfs"
OUT_DIR = HERE / "ground-truth" / "markdown"
PDF2MD = Path.home() / ".cargo" / "bin" / "pdf2md"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--force", action="store_true",
                        help="Re-process PDFs that already have a .md file")
    parser.add_argument("--provider", default="openai")
    parser.add_argument("--model", default="gpt-4.1-nano")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    pdfs = sorted(p for p in PDF_DIR.glob("*.pdf") if p.is_file())
    if not pdfs:
        print(f"No PDFs found in {PDF_DIR}", file=sys.stderr)
        return 1

    total = len(pdfs)
    skipped = processed = errors = 0
    t_start = time.perf_counter()

    for i, pdf in enumerate(pdfs, 1):
        out = OUT_DIR / f"{pdf.stem}.md"
        if out.exists() and not args.force:
            skipped += 1
            print(f"[{i}/{total}] skip  {pdf.name}")
            continue

        print(f"[{i}/{total}] {pdf.name} … ", end="", flush=True)
        t0 = time.perf_counter()
        result = subprocess.run(
            [str(PDF2MD), str(pdf), "-o", str(out),
             "--provider", args.provider, "--model", args.model, "-c", "10"],
            capture_output=True, text=True, timeout=300,
        )
        elapsed = time.perf_counter() - t0

        if result.returncode != 0:
            print(f"ERROR ({elapsed:.1f}s)")
            print(f"  stderr: {result.stderr.strip()[:200]}", file=sys.stderr)
            errors += 1
            out.unlink(missing_ok=True)
        else:
            chars = out.stat().st_size
            print(f"{chars:,} bytes  ({elapsed:.1f}s)")
            processed += 1

    total_time = time.perf_counter() - t_start
    print(f"\nDone in {total_time:.0f}s: {processed} processed, "
          f"{skipped} skipped, {errors} errors")
    return 0 if errors == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
