/**
 * Node adapter for odl-bench / CI — same OcrBackend interface as the browser,
 * backed by onnxruntime-node when available, else a null/degraded path.
 *
 * Uses edgeparse-core via the published WASM package when present; for the
 * bench harness we prefer the existing wasm-pack pkg + optional host OCR.
 */

import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import type { OcrBackend, OcrRequest, OcrWord } from '../ocr/types.js';
import { NullOcrBackend } from '../ocr/onnx-backend.js';
import { OcrResultCache } from '../ocr/types.js';
import type { ParseOptions, ParseResult, ResultMeta } from '../types.js';

const require = createRequire(import.meta.url);

export interface NodeParseOptions extends ParseOptions {
  /** Absolute path or URL to edgeparse_wasm JS glue. */
  wasmModule?: string;
  /** Optional host OCR backend (e.g. ppu-paddle-ocr bridge). */
  ocr?: OcrBackend;
}

export class NodeOcrBackend implements OcrBackend {
  readonly name = 'node-onnx';
  private inner: OcrBackend;

  constructor(inner?: OcrBackend) {
    this.inner = inner ?? new NullOcrBackend();
  }

  async ready(): Promise<void> {
    await this.inner.ready();
  }

  async recognize(req: OcrRequest): Promise<OcrWord[]> {
    return this.inner.recognize(req);
  }
}

/**
 * Parse PDF bytes on Node using the two-phase WASM session when available,
 * falling back to convert_to_string for older pkgs.
 */
export async function parsePdfNode(
  pdfBytes: Uint8Array,
  opts: NodeParseOptions = {},
): Promise<ParseResult> {
  const warnings: string[] = [];
  let quality: ParseResult['quality'] = 'full';
  const cache = new OcrResultCache();
  const ocr = opts.ocr ?? new NullOcrBackend();
  await ocr.ready();

  const timings: Record<string, number> = {};
  const t0 = performance.now();

  let wasm: {
    default?: (module_or_path?: unknown) => Promise<unknown>;
    ParseSession?: {
      open(
        bytes: Uint8Array,
        opts: Record<string, unknown>,
        cb: null,
      ): {
        candidates(): Array<{ id: number; width: number; height: number; hash: string }>;
        candidate_gray(id: number): Uint8Array;
        provide_ocr(id: number, words: OcrWord[]): void;
        finish(format: string): string;
      };
    };
    convert_to_string?: (
      bytes: Uint8Array,
      format?: string,
      pages?: string,
      readingOrder?: string,
      tableMethod?: string,
    ) => string;
    version?: () => string;
  };

  try {
    if (opts.wasmModule) {
      wasm = await import(opts.wasmModule);
    } else {
      wasm = require('edgeparse-wasm');
    }
    if (typeof wasm.default === 'function') {
      await wasm.default();
    }
  } catch (err) {
    throw new Error(
      `Failed to load edgeparse-wasm: ${err instanceof Error ? err.message : String(err)}`,
    );
  }

  const format = opts.format ?? 'markdown';
  let output: string;
  let imagesTotal = 0;
  let imagesOcred = 0;

  if (wasm.ParseSession) {
    const rasterTableOcr = opts.enableOcr !== false;
    const session = wasm.ParseSession.open(
      pdfBytes,
      {
        pages: opts.pages,
        readingOrder: opts.readingOrder,
        tableMethod: opts.tableMethod,
        fileName: opts.fileName ?? 'document.pdf',
        rasterTableOcr,
      },
      null,
    );
    const candidates = session.candidates();
    imagesTotal = candidates.length;
    const tOcr = performance.now();
    if (rasterTableOcr) {
      for (const cand of candidates) {
        const cached = cache.get(cand.hash);
        let words: OcrWord[];
        if (cached) {
          words = cached;
        } else {
          const gray = session.candidate_gray(cand.id);
          try {
            words = await ocr.recognize({
              id: String(cand.id),
              hash: cand.hash,
              width: cand.width,
              height: cand.height,
              gray,
            });
            cache.set(cand.hash, words);
            if (words.length) imagesOcred += 1;
          } catch (err) {
            words = [];
            quality = 'degraded';
            warnings.push(
              `OCR failed for ${cand.id}: ${err instanceof Error ? err.message : String(err)}`,
            );
          }
        }
        session.provide_ocr(cand.id, words);
      }
    }
    timings.ocrMs = performance.now() - tOcr;
    if (imagesTotal > 0 && imagesOcred === 0) quality = 'degraded';
    output = session.finish(format);
  } else if (wasm.convert_to_string) {
    warnings.push('ParseSession unavailable; using legacy convert_to_string');
    quality = 'degraded';
    output = wasm.convert_to_string(
      pdfBytes,
      format,
      opts.pages,
      opts.readingOrder,
      opts.tableMethod,
    );
  } else {
    throw new Error('edgeparse-wasm has neither ParseSession nor convert_to_string');
  }

  timings.totalMs = performance.now() - t0;
  const meta: ResultMeta = {
    wasmVersion: wasm.version?.() ?? 'unknown',
    models: [],
    timingsMs: timings,
    imagesTotal,
    imagesOcred,
    cacheHits: cache.hits,
    backend: 'node',
    tier: 'off',
  };

  const result: ParseResult = { warnings, quality, meta };
  if (format === 'markdown') result.markdown = output;
  else if (format === 'html') result.html = output;
  else if (format === 'text') result.text = output;
  else result.json = output;
  return result;
}

export async function parsePdfFile(
  path: string,
  opts: NodeParseOptions = {},
): Promise<ParseResult> {
  const buf = await readFile(path);
  return parsePdfNode(new Uint8Array(buf), { ...opts, fileName: path });
}

export { NullOcrBackend } from '../ocr/onnx-backend.js';
export { NodePpocrBackend } from './ppocr.js';
export type { NodePpocrOptions } from './ppocr.js';
export type { OcrBackend, OcrWord, ParseResult };
