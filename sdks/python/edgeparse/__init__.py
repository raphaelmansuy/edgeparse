"""edgeparse — High-performance PDF extraction (Rust engine)."""

from __future__ import annotations

from pathlib import Path
from typing import List, Optional, Tuple, Union

from edgeparse._edgeparse import (
    convert as _convert,
    convert_file as _convert_file,
    version as _version,
)

PathLike = Union[str, Path]


def convert(
    input_path: PathLike,
    *,
    format: str = "markdown",
    pages: Optional[str] = None,
    password: Optional[str] = None,
    reading_order: str = "xycut",
    table_method: str = "default",
    image_output: str = "off",
    hybrid: str = "off",
    hybrid_mode: str = "auto",
) -> str:
    """Convert a PDF file and return the extracted content as a string.

    Parameters
    ----------
    input_path:
        Path to the PDF file.
    format:
        Output format. Valid values: ``markdown``, ``json``, ``html``, ``text``.
        Default: ``markdown``.
    pages:
        Optional page range string, e.g. ``"1,3,5-7"``.
    password:
        Optional password for encrypted PDFs.
    reading_order:
        Reading order algorithm. ``"xycut"`` (default) or ``"off"``.
    table_method:
        Table detection method. ``"default"`` or ``"cluster"``.
    image_output:
        Image output mode. ``"off"`` (default), ``"embedded"``, or ``"external"``.
    hybrid:
        Hybrid OCR backend. ``"off"`` (default) or ``"docling-fast"``.
        ``"docling-fast"`` enables OCR recovery of image-based text and tables
        and requires ``pdftoppm`` and ``tesseract`` (and optionally ``rapidocr``)
        to be available on the host.
    hybrid_mode:
        Hybrid triage mode. ``"auto"`` (default) routes complex pages to OCR;
        ``"full"`` runs OCR on every page. Only applies when ``hybrid`` is enabled.

    Returns
    -------
    str
        The extracted content in the requested format.
    """
    return _convert(
        str(input_path),
        format=format,
        pages=pages,
        password=password,
        reading_order=reading_order,
        table_method=table_method,
        image_output=image_output,
        hybrid=hybrid,
        hybrid_mode=hybrid_mode,
    )


def convert_file(
    input_path: PathLike,
    output_dir: PathLike = "output",
    *,
    format: str = "markdown",
    pages: Optional[str] = None,
    password: Optional[str] = None,
    hybrid: str = "off",
    hybrid_mode: str = "auto",
) -> str:
    """Convert a PDF file and write the output to a file.

    Parameters
    ----------
    input_path:
        Path to the PDF file.
    output_dir:
        Directory to write output files. Created if it does not exist.
    format:
        Output format. Valid values: ``markdown``, ``json``, ``html``, ``text``.
        Default: ``markdown``.
    pages:
        Optional page range string, e.g. ``"1,3,5-7"``.
    password:
        Optional password for encrypted PDFs.
    hybrid:
        Hybrid OCR backend. ``"off"`` (default) or ``"docling-fast"``.
    hybrid_mode:
        Hybrid triage mode. ``"auto"`` (default) or ``"full"``.

    Returns
    -------
    str
        Path to the created output file.
    """
    return _convert_file(
        str(input_path),
        str(output_dir),
        format=format,
        pages=pages,
        password=password,
        hybrid=hybrid,
        hybrid_mode=hybrid_mode,
    )


def version() -> str:
    """Return the edgeparse version string."""
    return _version()


__all__ = ["convert", "convert_file", "version"]
__version__ = _version()
