"""Type definitions for edgeparse."""

from __future__ import annotations

from typing import Optional


# Valid output format strings
FORMATS = ("markdown", "json", "html", "text")

# Valid reading order algorithms
READING_ORDERS = ("xycut", "off")

# Valid table detection methods
TABLE_METHODS = ("default", "cluster")

# Valid image output modes
IMAGE_OUTPUTS = ("off", "embedded", "external")

# Valid hybrid OCR backends
HYBRID_BACKENDS = ("off", "docling-fast")

# Valid hybrid triage modes
HYBRID_MODES = ("auto", "full")
