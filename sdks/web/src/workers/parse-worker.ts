/**
 * Parse worker — owns the WASM ParseSession (CPU-bound, synchronous).
 *
 * Protocol (main ↔ worker):
 *   {type:'init', wasmUrl?} → {type:'ready', version}
 *   {type:'open', jobId, bytes, opts} → progress + {type:'planned', jobId, candidates}
 *   {type:'gray', jobId, id} → {type:'gray', jobId, id, gray, width, height, hash}
 *   {type:'ocr', jobId, id, words} → {type:'ocr-ack', jobId, id}
 *   {type:'finish', jobId, format, backendMarkdown?} → {type:'done', jobId, result}
 *   {type:'abort', jobId}
 */

/// <reference lib="webworker" />

type CandidateMeta = {
  id: number;
  page: number;
  width: number;
  height: number;
  hash: string;
};

type SessionLike = {
  candidates(): CandidateMeta[];
  candidate_gray(id: number): Uint8Array;
  provide_ocr(id: number, words: unknown): void;
  finish(format: string): string;
  finishAll(): {
    json: string;
    markdown: string;
    html: string;
    text: string;
  };
  finish_hybrid(backendMd: string | null | undefined, format: string): string;
};

type WasmModule = {
  default: (input?: RequestInfo | URL | BufferSource | WebAssembly.Module) => Promise<unknown>;
  ParseSession: {
    open(
      bytes: Uint8Array,
      opts: Record<string, unknown>,
      onProgress: ((phase: string, done: number, total: number) => void) | null,
    ): SessionLike;
  };
  version: () => string;
};

let wasm: WasmModule | null = null;
let session: SessionLike | null = null;
let activeJob: string | null = null;

async function loadWasm(wasmUrl?: string): Promise<WasmModule> {
  // Dynamic import so bundlers can tree-shake / alias edgeparse-wasm.
  const mod = (await import('edgeparse-wasm')) as unknown as WasmModule;
  await mod.default(wasmUrl || undefined);
  return mod;
}

self.onmessage = async (ev: MessageEvent) => {
  const msg = ev.data as Record<string, unknown>;
  try {
    switch (msg.type) {
      case 'init': {
        wasm = await loadWasm(msg.wasmUrl as string | undefined);
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'ready',
          version: wasm.version(),
        });
        break;
      }
      case 'open': {
        if (!wasm) throw new Error('WASM not initialized');
        activeJob = msg.jobId as string;
        const bytes = msg.bytes as Uint8Array;
        const opts = (msg.opts as Record<string, unknown>) ?? {};
        session = wasm.ParseSession.open(bytes, opts, (phase, done, total) => {
          (self as DedicatedWorkerGlobalScope).postMessage({
            type: 'progress',
            jobId: activeJob,
            phase,
            done,
            total,
          });
        });
        const candidates = session.candidates();
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'planned',
          jobId: activeJob,
          candidates,
        });
        break;
      }
      case 'gray': {
        if (!session) throw new Error('no session');
        const id = msg.id as number;
        const gray = session.candidate_gray(id);
        const candidates = session.candidates();
        const meta = candidates.find((c) => c.id === id);
        (self as DedicatedWorkerGlobalScope).postMessage(
          {
            type: 'gray',
            jobId: msg.jobId,
            id,
            gray,
            width: meta?.width ?? 0,
            height: meta?.height ?? 0,
            hash: meta?.hash ?? '',
          },
          [gray.buffer as ArrayBuffer],
        );
        break;
      }
      case 'ocr': {
        if (!session) throw new Error('no session');
        session.provide_ocr(msg.id as number, msg.words);
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'ocr-ack',
          jobId: msg.jobId,
          id: msg.id,
        });
        break;
      }
      case 'finish': {
        if (!session) throw new Error('no session');
        const format = (msg.format as string) ?? 'markdown';
        const backend = msg.backendMarkdown as string | null | undefined;
        if (format === 'all' && typeof session.finishAll === 'function') {
          const raw = session.finishAll() as unknown;
          const all =
            raw instanceof Map
              ? {
                  json: String(raw.get('json') ?? ''),
                  markdown: String(raw.get('markdown') ?? ''),
                  html: String(raw.get('html') ?? ''),
                  text: String(raw.get('text') ?? ''),
                }
              : (raw as {
                  json: string;
                  markdown: string;
                  html: string;
                  text: string;
                });
          (self as DedicatedWorkerGlobalScope).postMessage({
            type: 'done',
            jobId: msg.jobId,
            format: 'all',
            result: all,
          });
        } else {
          const result =
            backend != null && backend !== undefined
              ? session.finish_hybrid(backend, format)
              : session.finish(format);
          (self as DedicatedWorkerGlobalScope).postMessage({
            type: 'done',
            jobId: msg.jobId,
            format,
            result,
          });
        }
        session = null;
        activeJob = null;
        break;
      }
      case 'abort': {
        session = null;
        activeJob = null;
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'aborted',
          jobId: msg.jobId,
        });
        break;
      }
      default:
        break;
    }
  } catch (err) {
    (self as DedicatedWorkerGlobalScope).postMessage({
      type: 'error',
      jobId: msg.jobId ?? activeJob,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};

export {};
