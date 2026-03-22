"""Shared test fixtures for edgeparse tests."""

from __future__ import annotations

import pathlib
import tempfile

import pytest


@pytest.fixture
def fixtures_dir() -> pathlib.Path:
    """Return the path to the test fixtures directory."""
    root = pathlib.Path(__file__).resolve().parents[3]  # edgeparse repo root
    fixtures = root / "tests" / "fixtures"
    return fixtures


@pytest.fixture
def sample_pdf(fixtures_dir: pathlib.Path) -> pathlib.Path:
    """Return the path to first PDF test fixture if available."""
    pdfs = list(fixtures_dir.glob("*.pdf"))
    if not pdfs:
        pytest.skip("No PDF fixtures found in tests/fixtures/")
    return pdfs[0]


@pytest.fixture
def tmp_output_dir() -> pathlib.Path:
    """Create a temporary output directory for test artifacts."""
    with tempfile.TemporaryDirectory(prefix="edgeparse_test_") as tmp:
        yield pathlib.Path(tmp)
