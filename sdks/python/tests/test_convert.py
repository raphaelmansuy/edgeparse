"""Tests for the edgeparse Python SDK."""

from __future__ import annotations

import pathlib
import tempfile

import pytest

import edgeparse


class TestVersion:
    """Tests for the version function."""

    def test_version_returns_string(self):
        v = edgeparse.version()
        assert isinstance(v, str)
        assert len(v) > 0

    def test_version_attribute(self):
        assert hasattr(edgeparse, "__version__")
        assert edgeparse.__version__ == edgeparse.version()


class TestConvert:
    """Tests for the convert function."""

    def test_convert_returns_string(self, sample_pdf: pathlib.Path):
        result = edgeparse.convert(str(sample_pdf), format="text")
        assert isinstance(result, str)
        assert len(result) > 0

    def test_convert_markdown(self, sample_pdf: pathlib.Path):
        result = edgeparse.convert(str(sample_pdf), format="markdown")
        assert isinstance(result, str)

    def test_convert_json(self, sample_pdf: pathlib.Path):
        result = edgeparse.convert(str(sample_pdf), format="json")
        assert isinstance(result, str)
        # JSON output should be valid JSON
        import json
        parsed = json.loads(result)
        assert isinstance(parsed, (dict, list))

    def test_convert_html(self, sample_pdf: pathlib.Path):
        result = edgeparse.convert(str(sample_pdf), format="html")
        assert isinstance(result, str)

    def test_convert_file_not_found(self):
        with pytest.raises(Exception, match="File not found"):
            edgeparse.convert("/nonexistent/path.pdf")

    def test_convert_invalid_format(self, sample_pdf: pathlib.Path):
        with pytest.raises(Exception, match="Unknown format"):
            edgeparse.convert(str(sample_pdf), format="invalid_format")

    def test_convert_accepts_path_object(self, sample_pdf: pathlib.Path):
        result = edgeparse.convert(sample_pdf, format="text")
        assert isinstance(result, str)


class TestConvertFile:
    """Tests for the convert_file function."""

    def test_convert_file_creates_output(
        self, sample_pdf: pathlib.Path, tmp_output_dir: pathlib.Path
    ):
        out_path = edgeparse.convert_file(
            str(sample_pdf), str(tmp_output_dir), format="markdown"
        )
        assert pathlib.Path(out_path).exists()
        assert out_path.endswith(".md")

    def test_convert_file_json(
        self, sample_pdf: pathlib.Path, tmp_output_dir: pathlib.Path
    ):
        out_path = edgeparse.convert_file(
            str(sample_pdf), str(tmp_output_dir), format="json"
        )
        assert pathlib.Path(out_path).exists()
        assert out_path.endswith(".json")

    def test_convert_file_creates_dir(self, sample_pdf: pathlib.Path):
        with tempfile.TemporaryDirectory() as tmp:
            new_dir = pathlib.Path(tmp) / "nested" / "output"
            out_path = edgeparse.convert_file(
                str(sample_pdf), str(new_dir), format="text"
            )
            assert pathlib.Path(out_path).exists()


class TestAllExports:
    """Test that all public exports are present."""

    def test_all_exports(self):
        assert "convert" in edgeparse.__all__
        assert "convert_file" in edgeparse.__all__
        assert "version" in edgeparse.__all__
