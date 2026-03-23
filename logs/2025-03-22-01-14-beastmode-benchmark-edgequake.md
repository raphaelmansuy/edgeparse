# Task Log — 2025-03-22 01:14 — Benchmark: Marker fix + EdgeQuake integration

## Actions
- Investigated Marker 0.0 score → root cause: stale report (Marker worked, 22:30 run had evaluation.json with overall=0.8691)
- Discovered concurrent benchmark sessions (s004 + s018) causing MPS `torch.AcceleratorError` in Surya's `unpack_qkv_with_mask`  
- Fixed `pdf_parser_marker.py`: added `TORCH_DEVICE=cpu` to prevent all MPS crashes, timeout raised to 600s
- Created `benchmark/src/pdf_parser_edgequake.py` — uses `pdf2md` CLI (edgequake-pdf2md v0.5.0) with `--provider openai --model gpt-4.1-nano`
- Registered edgequake in `engine_registry.py` and `compare_all.py`
- Ran full benchmark across 7 engines (EdgeParse, Docling, OpenDataLoader, PyMuPDF4LLM, MarkItDown, EdgeQuake, Marker)
- Fixed `--no-run` mode to bypass install checks when loading existing evaluation files
- Generated final report: `reports/benchmark-20260322-011429.{html,json}`

## Final Results (7 engines, 200 docs each)
| Rank | Engine | Overall | NID | TEDS | MHS | s/doc |
|---|---|---|---|---|---|---|
| 1 | Docling | 0.8823 | 0.8990 | 0.8871 | 0.8243 | 1.27 |
| 2 | EdgeParse | 0.8481 | 0.9049 | 0.5808 | 0.7693 | 0.007 |
| 3 | Marker | 0.8463 | 0.8660 | 0.8245 | 0.7936 | 30.3 |
| 4 | OpenDataLoader | 0.8436 | 0.9119 | 0.4942 | 0.7597 | 0.053 |
| 5 | PyMuPDF4LLM | 0.8330 | 0.8884 | 0.5399 | 0.7737 | 0.72 |
| 6 | EdgeQuake (NEW) | 0.8277 | 0.8784 | 0.7952 | 0.6849 | 6.73 |
| 7 | MarkItDown | 0.5885 | 0.8437 | 0.2729 | 0.0000 | 0.20 |

## Decisions
- MinerU excluded (only 1 doc in evaluation.json — needs fresh run)
- `--no-run` mode now bypasses install checks to allow report generation from any evaluation.json files
- Marker CPU fix is future-safe but slow (~30s/doc vs. expected); existing evaluation.json from prior run used

## Next Steps
- Run MinerU separately when GPU memory is available
- Consider running Marker on CPU-only machine for fresh complete evaluation
- EdgeQuake at 0.8277 is competitive with VLM approach — good table accuracy (#3 TEDS)

## Lessons
- Always kill competing benchmark processes before launching a new one (use `lsof`/`ps` to verify)
- `--no-run` should skip installation checks to be useful for post-hoc reporting
- Two concurrent Surya/MPS processes cause `index out of bounds` errors that look engine-specific but are actually resource conflicts
