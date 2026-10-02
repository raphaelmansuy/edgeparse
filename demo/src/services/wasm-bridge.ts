/**
 * WASM bridge — EdgeParse Web SDK (`@edgeparse/web`).
 *
 * Observable engine / model / job state; two-phase OCR with consent.
 */

import { EdgeParse, type ParseResult, type Snapshot } from '@edgeparse/web';
import type { PdfDocument, OutputFormat } from '../types';
import { store } from '../state';
import { showModelConsent } from '../components/consent-dialog';

// Vite must emit workers as .js — `new URL('…worker.ts')` copies the .ts asset
// as-is, and GitHub Pages serves .ts as video/mp2t (breaks module workers).
import parseWorkerUrl from '../../../sdks/web/src/workers/parse-worker.ts?worker&url';
import ocrWorkerUrl from '../../../sdks/web/src/workers/ocr-worker.ts?worker&url';

export type FormatCache = Record<OutputFormat, string>;

let client: EdgeParse | null = null;
let clientPromise: Promise<EdgeParse> | null = null;
let lastSnapshot: Snapshot | null = null;
let lastResult: ParseResult | null = null;

/** Consent: accessible dialog before downloading OCR models. */
async function onBeforeDownload(model: {
  id: string;
  bytes: number;
}): Promise<boolean> {
  const mb = (model.bytes / (1024 * 1024)).toFixed(1);
  const detail = { id: model.id, mb, accepted: false as boolean };
  window.dispatchEvent(new CustomEvent('edgeparse:model-consent', { detail }));
  if (detail.accepted) return true;
  return showModelConsent({ id: model.id, mb });
}

async function getClient(): Promise<EdgeParse> {
  if (client) return client;
  if (!clientPromise) {
    store.set('wasmStatus', 'loading');
    clientPromise = EdgeParse.create({
      models: 'lazy',
      ocr: 'small',
      onBeforeDownload,
      parseWorkerUrl,
      ocrWorkerUrl,
    })
      .then((ep) => {
        client = ep;
        ep.subscribe(() => {
          lastSnapshot = ep.getSnapshot();
          const snap = lastSnapshot;
          if (snap.engine.state === 'ready') store.set('wasmStatus', 'ready');
          if (snap.engine.state === 'error' || snap.engine.state === 'unsupported') {
            store.set('wasmStatus', 'error');
            store.set(
              'errorMessage',
              snap.engine.error ?? 'WASM engine unavailable',
            );
          }
          window.dispatchEvent(
            new CustomEvent('edgeparse:snapshot', { detail: snap }),
          );
        });
        window.addEventListener('edgeparse:model-retry', ((e: CustomEvent<{ id: string }>) => {
          void ep.models.ensure(e.detail.id).catch(() => {
            /* surfaced via snapshot failed state */
          });
        }) as EventListener);
        store.set('wasmStatus', 'ready');
        return ep;
      })
      .catch((err) => {
        store.set('wasmStatus', 'error');
        store.set(
          'errorMessage',
          `WASM init failed: ${err instanceof Error ? err.message : String(err)}`,
        );
        throw err;
      });
  }
  return clientPromise;
}

/** Pre-warm WASM so it is ready before the first upload. */
export function ensureWasm(): void {
  void getClient();
}

export function getLastSnapshot(): Snapshot | null {
  return lastSnapshot;
}

/**
 * Parse a PDF via the SDK (parse worker + optional OCR worker).
 */
export async function parsePdf(
  bytes: Uint8Array,
): Promise<{ document: PdfDocument; cache: FormatCache; result: ParseResult }> {
  const ep = await getClient();
  store.set('parseStatus', 'parsing');
  store.set('errorMessage', null);

  try {
    const job = ep.parse(bytes, {
      format: 'all',
      wantAllFormats: true,
      tableMethod: 'cluster',
      fileName: store.get('fileName') || 'uploaded.pdf',
      enableOcr: store.get('enableOcr'),
    });

    job.on('progress', (p) => {
      window.dispatchEvent(
        new CustomEvent('edgeparse:job-progress', { detail: p }),
      );
    });

    const result = await job.result;
    const cache: FormatCache = {
      json: result.json ?? '',
      markdown: result.markdown ?? '',
      html: result.html ?? '',
      text: result.text ?? '',
    };

    let document: PdfDocument;
    try {
      document = (result.document as PdfDocument) ?? (JSON.parse(cache.json) as PdfDocument);
    } catch {
      document = {
        file_name: store.get('fileName') || 'uploaded.pdf',
        number_of_pages: 0,
        kids: [],
      } as unknown as PdfDocument;
    }

    if (result.quality === 'degraded') {
      store.set(
        'errorMessage',
        `Parsed with degraded OCR quality. ${result.warnings.join(' ')}`.trim(),
      );
    }

    lastResult = result;
    window.dispatchEvent(
      new CustomEvent('edgeparse:parse-result', {
        detail: { quality: result.quality, warnings: result.warnings },
      }),
    );

    store.set('parseStatus', 'done');
    return { document, cache, result };
  } catch (err) {
    store.set('parseStatus', 'error');
    throw err;
  }
}

export function getLastResult(): ParseResult | null {
  return lastResult;
}
