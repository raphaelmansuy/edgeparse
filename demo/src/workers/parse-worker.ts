/**
 * parse-worker.ts — WASM parsing in a dedicated worker thread.
 *
 * Running convert() in a worker keeps the main thread (and the UI) fully
 * responsive during PDF parsing, which can take several seconds for large docs.
 */
/// <reference lib="webworker" />

import wasmInit, { convert, convert_to_string } from '@edgeparse/edgeparse-wasm';

export type ParseRequest = {
  type: 'parse';
  id: string;
  bytes: Uint8Array;
};

export type ParseResponse =
  | { type: 'ready' }
  | { type: 'result'; id: string; ok: true; document: unknown; cache: Record<string, string> }
  | { type: 'result'; id: string; ok: false; error: string };

// Pre-warm WASM as soon as the worker is created so it is ready before the
// first parse request arrives.
const initPromise: Promise<void> = wasmInit().then(() => {
  (self as DedicatedWorkerGlobalScope).postMessage({ type: 'ready' } satisfies ParseResponse);
});

(self as DedicatedWorkerGlobalScope).onmessage = async (e: MessageEvent<ParseRequest>) => {
  const { type, id, bytes } = e.data;
  if (type !== 'parse') return;

  try {
    await initPromise; // no-op once already resolved
    const document = convert(bytes, 'json');
    const cache: Record<string, string> = {
      json:      convert_to_string(bytes, 'json'),
      markdown:  convert_to_string(bytes, 'markdown'),
      html:      convert_to_string(bytes, 'html'),
      text:      convert_to_string(bytes, 'text'),
    };
    (self as DedicatedWorkerGlobalScope).postMessage(
      { type: 'result', id, ok: true, document, cache } satisfies ParseResponse,
    );
  } catch (err: unknown) {
    (self as DedicatedWorkerGlobalScope).postMessage(
      { type: 'result', id, ok: false, error: String(err) } satisfies ParseResponse,
    );
  }
};
