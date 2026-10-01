# @edgeparse/web

Observable PDF parsing for the browser — WASM plan → async PP-OCR → assemble.

## Install

```bash
npm install @edgeparse/web edgeparse-wasm
```

## Quick start

```ts
import { EdgeParse } from '@edgeparse/web';

const ep = await EdgeParse.create({
  models: 'lazy',          // download OCR models only when needed
  ocr: 'small',
  onBeforeDownload: async (model) => {
    // show consent UI; return false to degrade without OCR
    return confirm(`Download ${model.id} (~${(model.bytes / 1e6).toFixed(1)} MB)?`);
  },
});

ep.subscribe(() => {
  const snap = ep.getSnapshot();
  // snap.engine / snap.models / snap.jobs — React useSyncExternalStore friendly
});

const job = ep.parse(file, { format: 'markdown', tableMethod: 'cluster' });
job.on('progress', (p) => console.log(p.fraction, p.label));
const result = await job.result;
// result.quality: 'full' | 'degraded' | 'skipped'
```

## Architecture

1. **Parse worker** — `ParseSession` (Rust/WASM): extract candidates, finish with OCR words.
2. **OCR worker** — onnxruntime-web (WebGPU → WASM+SIMD), PP-OCRv6 tiers.
3. **ModelManager** — pinned manifest, OPFS/Cache API, resume, sha256, Web Locks.

Missing OCR never fails a parse — cells fall back to PDF text and `quality: "degraded"`.

## CSP

See [docs/CSP.md](./docs/CSP.md).

## Node / benchmarks

```ts
import { parsePdfFile } from '@edgeparse/web/node';
const result = await parsePdfFile('./doc.pdf', { format: 'markdown' });
```
