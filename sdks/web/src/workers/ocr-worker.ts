/**
 * OCR worker — runs onnxruntime-web (WebGPU → WASM+SIMD) off the main thread.
 *
 * Protocol:
 *   {type:'init', providers?, models?: {detection, recognition, dictionary}} → {type:'ready'}
 *   {type:'recognize', reqId, id, hash, width, height, gray} → {type:'words', reqId, id, hash, words}
 */

/// <reference lib="webworker" />

import { NullOcrBackend, OnnxOcrBackend } from '../ocr/onnx-backend.js';
import type { OcrBackend, OcrWord } from '../ocr/types.js';
import { OcrResultCache } from '../ocr/types.js';

let backend: OcrBackend = new NullOcrBackend();
const cache = new OcrResultCache();

self.onmessage = async (ev: MessageEvent) => {
  const msg = ev.data as Record<string, unknown>;
  try {
    switch (msg.type) {
      case 'init': {
        const models = msg.models as
          | {
              detection?: Uint8Array;
              recognition?: Uint8Array;
              dictionary?: string;
            }
          | undefined;
        const providers = (msg.providers as Array<'webgpu' | 'wasm'>) ?? [
          'webgpu',
          'wasm',
        ];
        backend = new OnnxOcrBackend({ models, providers });
        await backend.ready();
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'ready',
          backend: backend.name,
          reqId: msg.reqId,
        });
        break;
      }
      case 'recognize': {
        const hash = msg.hash as string;
        const cached = cache.get(hash);
        if (cached) {
          (self as DedicatedWorkerGlobalScope).postMessage({
            type: 'words',
            reqId: msg.reqId,
            id: msg.id,
            hash,
            words: cached,
            cacheHit: true,
          });
          break;
        }
        const words: OcrWord[] = await backend.recognize({
          id: String(msg.id),
          hash,
          width: msg.width as number,
          height: msg.height as number,
          gray: msg.gray as Uint8Array,
        });
        cache.set(hash, words);
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'words',
          reqId: msg.reqId,
          id: msg.id,
          hash,
          words,
          cacheHit: false,
        });
        break;
      }
      case 'stats': {
        (self as DedicatedWorkerGlobalScope).postMessage({
          type: 'stats',
          reqId: msg.reqId,
          hits: cache.hits,
          misses: cache.misses,
        });
        break;
      }
      default:
        break;
    }
  } catch (err) {
    (self as DedicatedWorkerGlobalScope).postMessage({
      type: 'error',
      reqId: msg.reqId,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};

export {};
