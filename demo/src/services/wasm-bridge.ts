/**
 * WASM bridge — PDF parsing via a dedicated Web Worker.
 *
 * Running convert() in a worker keeps the main thread fully responsive;
 * the synchronous WASM calls can take several seconds on large PDFs.
 */

import type { PdfDocument, OutputFormat } from '../types';
import { store } from '../state';

export type FormatCache = Record<OutputFormat, string>;

type WorkerResponse =
  | { type: 'ready' }
  | { type: 'result'; id: string; ok: true; document: PdfDocument; cache: FormatCache }
  | { type: 'result'; id: string; ok: false; error: string };

// Singleton worker — created once, reused for every parse request.
const worker = new Worker(
  new URL('../workers/parse-worker.ts', import.meta.url),
  { type: 'module' },
);

// Pending parse calls keyed by request ID.
const pending = new Map<string, { resolve: (v: { document: PdfDocument; cache: FormatCache }) => void; reject: (e: unknown) => void }>();

worker.addEventListener('message', (e: MessageEvent<WorkerResponse>) => {
  const msg = e.data;
  if (msg.type === 'ready') {
    store.set('wasmStatus', 'ready');
    return;
  }
  if (msg.type === 'result') {
    const p = pending.get(msg.id);
    if (!p) return;
    pending.delete(msg.id);
    if (msg.ok) {
      store.set('parseStatus', 'done');
      p.resolve({ document: msg.document, cache: msg.cache });
    } else {
      store.set('parseStatus', 'error');
      store.set('errorMessage', `Parse error: ${msg.error}`);
      p.reject(new Error(msg.error));
    }
  }
});

worker.addEventListener('error', (e) => {
  store.set('wasmStatus', 'error');
  store.set('errorMessage', `Worker error: ${e.message}`);
});

// Signal that WASM is initialising (worker pre-warms on creation).
store.set('wasmStatus', 'loading');

/** Pre-warm: call once at app start so WASM is ready before first upload. */
export function ensureWasm(): void {
  // Worker initialises on creation; nothing extra needed here.
  // This function is kept for backwards-compat call sites.
}

let _nextId = 0;

/**
 * Parse a PDF in the worker thread and return all format outputs.
 * The main thread remains fully responsive during this call.
 */
export function parsePdf(
  bytes: Uint8Array,
): Promise<{ document: PdfDocument; cache: FormatCache }> {
  store.set('parseStatus', 'parsing');
  const id = String(_nextId++);
  const promise = new Promise<{ document: PdfDocument; cache: FormatCache }>((resolve, reject) => {
    pending.set(id, { resolve, reject });
  });
  // Transfer a *copy* so the store's pdfBytes is not detached.
  worker.postMessage({ type: 'parse', id, bytes });
  return promise;
}
