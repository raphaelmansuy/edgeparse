"""CLI entry point — mirrors the Rust CLI but invoked via ``edgeparse`` console script."""

from __future__ import annotations

import argparse
import sys

from . import convert_file


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="edgeparse",
        description="Convert PDF files to Markdown, JSON, HTML, or plain text.",
    )
    parser.add_argument("input", nargs="+", help="PDF file(s) to convert")
    parser.add_argument(
        "-o", "--output-dir", default="output", help="Output directory (default: output)"
    )
    parser.add_argument(
        "-f",
        "--format",
        default="markdown",
        help="Output format: markdown, json, html, text (default: markdown)",
    )
    parser.add_argument("--pages", help="Page range, e.g. 1,3,5-7")
    parser.add_argument("-p", "--password", help="Password for encrypted PDFs")
    args = parser.parse_args()

    has_errors = False
    for input_path in args.input:
        try:
            out = convert_file(
                input_path,
                output_dir=args.output_dir,
                format=args.format,
                pages=args.pages,
                password=args.password,
            )
            print(out)
        except Exception as e:
            print(f"Error processing {input_path}: {e}", file=sys.stderr)
            has_errors = True

    if has_errors:
        sys.exit(1)


if __name__ == "__main__":
    main()
