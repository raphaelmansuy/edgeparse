import { pickBackend, pickDefaultTier, probeCapabilities } from './capabilities.js';
import { getDefaultManifest, ModelManager } from './models/index.js';
import { ClientState } from './state/client-state.js';
import { JOB_WEIGHTS, ProgressTracker } from './state/store.js';
import {
  EdgeParseError,
  type ClientEventMap,
  type EdgeParseOptions,
  type ModelTier,
  type ParseOptions,
  type ParseResult,
  type ResultMeta,
  type Snapshot,
} from './types.js';

type CandidateMeta = {
  id: number;
  page: number;
  width: number;
  height: number;
  hash: string;
  kind?: 'table' | 'textBlocks';
};

let jobSeq = 0;

export class ParseJob {
  readonly id: string;
  readonly result: Promise<ParseResult>;
  private resolveResult!: (r: ParseResult) => void;
  private rejectResult!: (e: unknown) => void;
  private aborted = false;
  private readonly progressHandlers = new Set<(p: import('./types.js').JobProgress) => void>();

  constructor(id: string) {
    this.id = id;
    this.result = new Promise<ParseResult>((resolve, reject) => {
      this.resolveResult = resolve;
      this.rejectResult = reject;
    });
  }

  on(event: 'progress', handler: (p: import('./types.js').JobProgress) => void): () => void {
    this.progressHandlers.add(handler);
    return () => this.progressHandlers.delete(handler);
  }

  /** @internal */
  _emitProgress(p: import('./types.js').JobProgress): void {
    for (const h of this.progressHandlers) h(p);
  }

  /** @internal */
  _resolve(r: ParseResult): void {
    this.resolveResult(r);
  }

  /** @internal */
  _reject(e: unknown): void {
    this.rejectResult(e);
  }

  abort(): void {
    this.aborted = true;
  }

  get isAborted(): boolean {
    return this.aborted;
  }
}

export class EdgeParse {
  readonly models: {
    preload: (tier?: ModelTier) => Promise<void>;
    ensure: (id: string) => Promise<void>;
  };

  private readonly state: ClientState;
  private readonly options: EdgeParseOptions;
  private readonly modelManager: ModelManager;
  private parseWorker: Worker | null = null;
  private ocrWorker: Worker | null = null;
  private wasmVersion = 'unknown';
  private backend: ResultMeta['backend'] = 'none';
  private tier: ModelTier | 'off' = 'small';
  private readyPromise: Promise<void>;

  private constructor(options: EdgeParseOptions, state: ClientState, mm: ModelManager) {
    this.options = options;
    this.state = state;
    this.modelManager = mm;
    this.models = {
      preload: async (tier) => {
        const t = tier ?? (this.tier === 'off' ? 'small' : this.tier);
        if (this.options.models === 'off' || this.tier === 'off') return;
        await this.modelManager.ensureTier(t);
      },
      ensure: async (id) => {
        await this.modelManager.ensure(id);
      },
    };
    this.readyPromise = this.boot();
  }

  static async create(options: EdgeParseOptions = {}): Promise<EdgeParse> {
    const state = new ClientState();
    const manifest = options.manifest ?? getDefaultManifest();
    const mm = new ModelManager({
      manifest,
      state,
      onBeforeDownload: options.onBeforeDownload,
    });
    const client = new EdgeParse(options, state, mm);
    await client.readyPromise;
    return client;
  }

  static async capabilities() {
    return probeCapabilities();
  }

  subscribe(listener: () => void): () => void {
    return this.state.store.subscribe(listener);
  }

  getSnapshot(): Snapshot {
    return this.state.store.getSnapshot();
  }

  on<K extends keyof ClientEventMap>(
    type: K,
    handler: (event: ClientEventMap[K]) => void,
  ): () => void {
    return this.state.on(type, handler);
  }

  parse(input: Blob | ArrayBuffer | Uint8Array | File, opts: ParseOptions = {}): ParseJob {
    const jobId = `job-${++jobSeq}`;
    const job = new ParseJob(jobId);
    this.state.patchJob(jobId, 'queued', { label: 'Queued', fraction: 0 });

    void this.runJob(job, input, opts).catch((err) => {
      const code =
        err instanceof EdgeParseError ? err.code : ('UNKNOWN' as const);
      const message = err instanceof Error ? err.message : String(err);
      this.state.failJob(jobId, code, message);
      job._reject(err);
    });

    if (opts.signal) {
      const onAbort = (): void => {
        job.abort();
        this.abortJob(jobId);
      };
      if (opts.signal.aborted) {
        onAbort();
      } else {
        opts.signal.addEventListener('abort', onAbort, { once: true });
        void job.result.finally(() => {
          opts.signal?.removeEventListener('abort', onAbort);
        });
      }
    }

    return job;
  }

  async dispose(): Promise<void> {
    this.parseWorker?.terminate();
    this.ocrWorker?.terminate();
    this.parseWorker = null;
    this.ocrWorker = null;
  }

  private async boot(): Promise<void> {
    this.state.setEngine('loading-wasm');
    const caps = await probeCapabilities();
    this.state.setCapabilities(caps);
    if (!caps.wasm) {
      this.state.setEngine('unsupported', 'WebAssembly not available');
      throw new EdgeParseError('WASM_UNSUPPORTED', 'WebAssembly not available');
    }
    this.backend = pickBackend(caps);
    if (this.options.ocr === 'off' || this.options.models === 'off') {
      this.tier = 'off';
    } else if (this.options.ocr) {
      this.tier = this.options.ocr;
    } else {
      this.tier = pickDefaultTier(caps);
    }

    if (typeof window !== 'undefined') {
      window.addEventListener('online', () => this.state.setOnline(true));
      window.addEventListener('offline', () => this.state.setOnline(false));
    }

    await this.spawnParseWorker();
    if (this.tier !== 'off') {
      await this.spawnOcrWorker();
      if (this.options.models === 'preload') {
        try {
          await this.modelManager.ensureTier(this.tier);
          await this.warmOcrModels();
        } catch {
          // Preload failure is non-fatal; lazy path may still work.
        }
      }
    }
    this.state.setEngine('ready');
  }

  private parseWorkerUrl(): URL {
    if (this.options.parseWorkerUrl instanceof URL) {
      return this.options.parseWorkerUrl;
    }
    if (typeof this.options.parseWorkerUrl === 'string') {
      // Absolute paths from bundlers (?worker&url) resolve against the document.
      return new URL(this.options.parseWorkerUrl, self.location?.href ?? import.meta.url);
    }
    // Concatenate so Vite/Rollup do not statically rewrite this to a .ts asset
    // (GitHub Pages serves .ts as video/mp2t and module workers fail).
    return new URL('./workers/' + 'parse-worker.js', import.meta.url);
  }

  private ocrWorkerUrl(): URL {
    if (this.options.ocrWorkerUrl instanceof URL) {
      return this.options.ocrWorkerUrl;
    }
    if (typeof this.options.ocrWorkerUrl === 'string') {
      return new URL(this.options.ocrWorkerUrl, self.location?.href ?? import.meta.url);
    }
    return new URL('./workers/' + 'ocr-worker.js', import.meta.url);
  }

  private async spawnParseWorker(): Promise<void> {
    this.parseWorker?.terminate();
    this.parseWorker = new Worker(this.parseWorkerUrl(), { type: 'module' });
    await new Promise<void>((resolve, reject) => {
      const w = this.parseWorker!;
      const timer = setTimeout(() => {
        w.removeEventListener('message', onMsg);
        reject(new Error('parse worker init timed out'));
      }, 30_000);
      const onMsg = (ev: MessageEvent) => {
        if (ev.data?.type === 'ready') {
          clearTimeout(timer);
          this.wasmVersion = ev.data.version ?? 'unknown';
          w.removeEventListener('message', onMsg);
          resolve();
        } else if (ev.data?.type === 'error') {
          clearTimeout(timer);
          w.removeEventListener('message', onMsg);
          reject(new Error(ev.data.error));
        }
      };
      w.addEventListener('message', onMsg);
      w.addEventListener('error', (e) => {
        clearTimeout(timer);
        reject(new Error(e.message || 'parse worker failed'));
      });
      w.postMessage({ type: 'init', wasmUrl: this.options.wasmUrl });
    });
  }

  private async spawnOcrWorker(): Promise<void> {
    this.ocrWorker?.terminate();
    this.ocrWorker = new Worker(this.ocrWorkerUrl(), { type: 'module' });
    const providers =
      this.backend === 'webgpu' ? (['webgpu', 'wasm'] as const) : (['wasm'] as const);
    await new Promise<void>((resolve) => {
      const w = this.ocrWorker!;
      const onMsg = (ev: MessageEvent) => {
        if (ev.data?.type === 'ready' || ev.data?.type === 'error') {
          w.removeEventListener('message', onMsg);
          resolve();
        }
      };
      w.addEventListener('message', onMsg);
      w.postMessage({ type: 'init', providers: [...providers] });
    });
  }

  private async tryWarmCachedOcrModels(): Promise<boolean> {
    if (!this.ocrWorker || this.tier === 'off') return false;
    if (this.options.models === 'manual' || this.options.models === 'off') return false;
    const entries = this.modelManager.modelsForTier(this.tier as ModelTier);
    const detEntry = entries.find((e) => e.kind === 'detection');
    const recEntry = entries.find((e) => e.kind === 'recognition');
    const dictEntry = entries.find((e) => e.kind === 'dictionary');
    if (!detEntry || !recEntry || !dictEntry) return false;
    const [det, rec, dict] = await Promise.all([
      this.modelManager.peekCached(detEntry.id),
      this.modelManager.peekCached(recEntry.id),
      this.modelManager.peekCached(dictEntry.id),
    ]);
    if (!det || !rec || !dict) return false;
    const dictionary = new TextDecoder().decode(dict.bytes);
    const reqId = `ocr-init-${Date.now()}`;
    await this.workerRequest(this.ocrWorker, {
      type: 'init',
      reqId,
      providers: this.backend === 'webgpu' ? ['webgpu', 'wasm'] : ['wasm'],
      models: {
        detection: det.bytes,
        recognition: rec.bytes,
        dictionary,
      },
    });
    return true;
  }

  private async warmOcrModels(): Promise<void> {
    if (!this.ocrWorker || this.tier === 'off') return;
    if (this.options.models === 'manual' || this.options.models === 'off') return;
    const entries = this.modelManager.modelsForTier(this.tier as ModelTier);
    const detEntry = entries.find((e) => e.kind === 'detection');
    const recEntry = entries.find((e) => e.kind === 'recognition');
    const dictEntry = entries.find((e) => e.kind === 'dictionary');
    if (!detEntry || !recEntry || !dictEntry) return;
    const [det, rec, dict] = await Promise.all([
      this.modelManager.ensure(detEntry.id),
      this.modelManager.ensure(recEntry.id),
      this.modelManager.ensure(dictEntry.id),
    ]);
    const dictionary = new TextDecoder().decode(dict.bytes);
    const reqId = `ocr-init-${Date.now()}`;
    await this.workerRequest(this.ocrWorker, {
      type: 'init',
      reqId,
      providers: this.backend === 'webgpu' ? ['webgpu', 'wasm'] : ['wasm'],
      models: {
        detection: det.bytes,
        recognition: rec.bytes,
        dictionary,
      },
    });
  }

  private abortJob(jobId: string): void {
    this.parseWorker?.postMessage({ type: 'abort', jobId });
    // Hard abort: sync wasm cannot be interrupted — respawn worker.
    void this.spawnParseWorker().then(() => {
      this.state.failJob(jobId, 'ABORTED', 'Aborted');
    });
  }

  private async runJob(
    job: ParseJob,
    input: Blob | ArrayBuffer | Uint8Array | File,
    opts: ParseOptions,
  ): Promise<void> {
    await this.readyPromise;
    if (!this.parseWorker) {
      throw new EdgeParseError('WASM_UNSUPPORTED', 'Parse worker missing');
    }

    const tracker = new ProgressTracker();
    const timings: Record<string, number> = {};
    const warnings: string[] = [];
    let quality: ParseResult['quality'] = 'full';
    let imagesTotal = 0;
    let imagesOcred = 0;
    let cacheHits = 0;

    const emit = (state: Parameters<ClientState['patchJob']>[1], label: string, frac?: number) => {
      const fraction = frac != null ? tracker.set(frac) : tracker.current;
      const p = this.state.patchJob(job.id, state, {
        label,
        fraction,
        ocrDone: imagesOcred,
        ocrTotal: imagesTotal,
        warnings: [...warnings],
      });
      job._emitProgress(p);
    };

    const bytes = await toUint8Array(input);
    // Copy before postMessage transfer — transferring the viewer's buffer
    // detaches it and PDF.js / later parses see an empty document.
    const workerBytes = new Uint8Array(bytes);
    const t0 = performance.now();
    emit('opening', 'Opening PDF', JOB_WEIGHTS.opening * 0.5);

    const rasterTableOcr =
      opts.enableOcr !== false && this.tier !== 'off';

    const planned = await this.workerRequest<{
      type: 'planned';
      candidates: CandidateMeta[];
    }>(this.parseWorker, {
      type: 'open',
      jobId: job.id,
      bytes: workerBytes,
      opts: {
        pages: opts.pages,
        readingOrder: opts.readingOrder,
        tableMethod: opts.tableMethod,
        rasterTableOcr,
        fileName:
          opts.fileName ??
          (typeof File !== 'undefined' && input instanceof File ? input.name : 'uploaded.pdf'),
      },
    }, (msg) => {
      if (msg.type === 'progress' && msg.phase === 'planning') {
        const done = Number(msg.done ?? 0);
        const total = Math.max(1, Number(msg.total ?? 1));
        emit(
          'planning',
          `Planning page ${done}/${total}`,
          JOB_WEIGHTS.opening + JOB_WEIGHTS.planning * (done / total),
        );
      }
    });

    timings.planMs = performance.now() - t0;
    const candidates = planned.candidates ?? [];
    imagesTotal = candidates.length;
    emit(
      'planning',
      candidates.length
        ? `Found ${candidates.length} OCR candidate(s)`
        : 'No OCR candidates',
      JOB_WEIGHTS.opening + JOB_WEIGHTS.planning,
    );

    if (job.isAborted) {
      throw new EdgeParseError('ABORTED', 'Aborted');
    }

    // OCR is optional. Native PDF text must still assemble immediately.
    // Only run OCR when models are already cached (or policy is preload).
    // Prompting for a download here used to stall parse until the user
    // accepted/declined, leaving the markdown pane empty.
    let ocrReady = false;
    if (
      rasterTableOcr &&
      candidates.length > 0 &&
      this.tier !== 'off' &&
      this.options.models !== 'off' &&
      this.options.models !== 'manual'
    ) {
      try {
        ocrReady = await this.tryWarmCachedOcrModels();
        if (!ocrReady) {
          quality = 'degraded';
          const hasTextBlocks = candidates.some((c) => c.kind === 'textBlocks');
          warnings.push(
            hasTextBlocks
              ? 'Page is mostly raster; download OCR models for full text.'
              : 'OCR models not cached — using PDF text only. Download models to OCR image tables.',
          );
        }
      } catch (err) {
        quality = 'degraded';
        warnings.push(
          err instanceof Error
            ? `OCR models unavailable: ${err.message}`
            : 'OCR models unavailable',
        );
      }
    } else if (candidates.length > 0 && this.options.models === 'manual') {
      quality = 'degraded';
      warnings.push('OCR models policy is manual — skipping download');
    } else if (!rasterTableOcr && candidates.length === 0) {
      // Explicit OCR off — no warning; born-digital text only.
    }

    const tOcr = performance.now();
    if (ocrReady) {
      for (let i = 0; i < candidates.length; i++) {
        if (job.isAborted) throw new EdgeParseError('ABORTED', 'Aborted');
        const cand = candidates[i]!;
        emit(
          'ocr',
          `OCR ${i + 1}/${candidates.length}`,
          JOB_WEIGHTS.opening +
            JOB_WEIGHTS.planning +
            JOB_WEIGHTS.ocr * ((i + 0.5) / Math.max(1, candidates.length)),
        );

        const grayMsg = await this.workerRequest<{
          type: 'gray';
          gray: Uint8Array;
          width: number;
          height: number;
          hash: string;
        }>(this.parseWorker, { type: 'gray', jobId: job.id, id: cand.id });

        let words: unknown[] = [];
        if (this.ocrWorker && this.tier !== 'off') {
          try {
            const ocrMsg = await this.workerRequest<{
              type: 'words';
              words: unknown[];
              cacheHit?: boolean;
            }>(this.ocrWorker, {
              type: 'recognize',
              reqId: `${job.id}-${cand.id}`,
              id: cand.id,
              hash: grayMsg.hash || cand.hash,
              width: grayMsg.width,
              height: grayMsg.height,
              gray: grayMsg.gray,
            });
            words = ocrMsg.words ?? [];
            if (ocrMsg.cacheHit) cacheHits += 1;
            if (words.length) imagesOcred += 1;
          } catch (err) {
            quality = 'degraded';
            warnings.push(
              `OCR failed for candidate ${cand.id}: ${
                err instanceof Error ? err.message : String(err)
              }`,
            );
          }
        }

        await this.workerRequest(this.parseWorker, {
          type: 'ocr',
          jobId: job.id,
          id: cand.id,
          words,
        });
      }
    } else if (candidates.length > 0) {
      quality = quality === 'full' ? 'degraded' : quality;
    }
    timings.ocrMs = performance.now() - tOcr;

    emit(
      'assembling',
      'Assembling document',
      JOB_WEIGHTS.opening + JOB_WEIGHTS.planning + JOB_WEIGHTS.ocr,
    );
    const tAsm = performance.now();
    const done = await this.workerRequest<{
      type: 'done';
      format: string;
      result: string | { json: string; markdown: string; html: string; text: string };
    }>(this.parseWorker, {
      type: 'finish',
      jobId: job.id,
      format: opts.format === 'all' || opts.wantAllFormats ? 'all' : (opts.format ?? 'markdown'),
      backendMarkdown: opts.backendMarkdown,
    });
    timings.assembleMs = performance.now() - tAsm;
    timings.totalMs = performance.now() - t0;

    const format = opts.format ?? 'markdown';
    const parseResult: ParseResult = {
      warnings,
      quality: candidates.length === 0 ? quality : wordsQuality(quality, imagesOcred, imagesTotal),
      meta: {
        wasmVersion: this.wasmVersion,
        models: this.modelManager
          .modelsForTier(this.tier === 'off' ? 'small' : this.tier)
          .map((m) => ({
            id: m.id,
            version: this.modelManager.getManifest().version,
            sha256: m.sha256,
          })),
        timingsMs: timings,
        imagesTotal,
        imagesOcred,
        cacheHits,
        backend: this.backend,
        tier: this.tier,
      },
    };
    const all = asFormatBundle(done.result);
    if (all) {
      parseResult.json = all.json;
      parseResult.markdown = all.markdown;
      parseResult.html = all.html;
      parseResult.text = all.text;
      try {
        parseResult.document = JSON.parse(all.json);
      } catch {
        /* ignore */
      }
    } else if (format === 'markdown') {
      parseResult.markdown = done.result as string;
    } else if (format === 'html') {
      parseResult.html = done.result as string;
    } else if (format === 'text') {
      parseResult.text = done.result as string;
    } else {
      parseResult.json = done.result as string;
    }

    emit('done', 'Done', 1);
    this.state.emit('job:done', { jobId: job.id, result: parseResult });
    job._resolve(parseResult);
  }

  private reqSeq = 0;

  private workerRequest<T extends { type: string }>(
    worker: Worker,
    payload: Record<string, unknown>,
    onProgress?: (msg: Record<string, unknown>) => void,
  ): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const reqId =
        (payload.reqId as string | undefined) ??
        (payload.jobId as string | undefined) ??
        `req-${++this.reqSeq}`;
      payload.reqId = reqId;
      const expectedType =
        payload.type === 'open'
          ? 'planned'
          : payload.type === 'gray'
            ? 'gray'
            : payload.type === 'ocr'
              ? 'ocr-ack'
              : payload.type === 'finish'
                ? 'done'
                : payload.type === 'recognize'
                  ? 'words'
                  : payload.type === 'init'
                    ? 'ready'
                    : null;

      const onMsg = (ev: MessageEvent) => {
        const msg = ev.data as Record<string, unknown>;
        if (onProgress && msg.type === 'progress' && msg.jobId === payload.jobId) {
          onProgress(msg);
          return;
        }
        const idMatch =
          msg.reqId === reqId ||
          (msg.jobId != null && msg.jobId === payload.jobId && expectedType === msg.type) ||
          (payload.type === 'gray' && msg.type === 'gray' && msg.id === payload.id) ||
          (payload.type === 'ocr' && msg.type === 'ocr-ack' && msg.id === payload.id) ||
          (payload.type === 'open' && msg.type === 'planned' && msg.jobId === payload.jobId) ||
          (payload.type === 'finish' && msg.type === 'done' && msg.jobId === payload.jobId);

        if (msg.type === 'error' && (msg.reqId === reqId || msg.jobId === payload.jobId)) {
          worker.removeEventListener('message', onMsg);
          reject(new Error(String(msg.error)));
          return;
        }
        if (msg.type === 'aborted' && msg.jobId === payload.jobId) {
          worker.removeEventListener('message', onMsg);
          reject(new EdgeParseError('ABORTED', 'Aborted'));
          return;
        }
        if (
          idMatch &&
          (expectedType == null || msg.type === expectedType || msg.type === 'ready')
        ) {
          worker.removeEventListener('message', onMsg);
          resolve(msg as T);
        }
      };
      worker.addEventListener('message', onMsg);
      const transfer: Transferable[] = [];
      if (payload.bytes instanceof Uint8Array) {
        transfer.push(payload.bytes.buffer);
      }
      if (payload.gray instanceof Uint8Array) {
        transfer.push(payload.gray.buffer);
      }
      if (
        payload.models &&
        typeof payload.models === 'object' &&
        payload.models !== null
      ) {
        const m = payload.models as Record<string, unknown>;
        if (m.detection instanceof Uint8Array) transfer.push(m.detection.buffer);
        if (m.recognition instanceof Uint8Array) transfer.push(m.recognition.buffer);
      }
      worker.postMessage(payload, transfer);
    });
  }
}

function asFormatBundle(
  value: unknown,
): { json: string; markdown: string; html: string; text: string } | null {
  if (!value || typeof value !== 'object') return null;
  const read = (key: string): string | undefined => {
    const rec = value as Record<string, unknown>;
    if (typeof rec[key] === 'string') return rec[key] as string;
    if (value instanceof Map) {
      const v = value.get(key);
      if (typeof v === 'string') return v;
    }
    return undefined;
  };
  const json = read('json');
  const markdown = read('markdown');
  const html = read('html');
  const text = read('text');
  if (
    json === undefined &&
    markdown === undefined &&
    html === undefined &&
    text === undefined
  ) {
    return null;
  }
  return {
    json: json ?? '',
    markdown: markdown ?? '',
    html: html ?? '',
    text: text ?? '',
  };
}

function wordsQuality(
  quality: ParseResult['quality'],
  ocred: number,
  total: number,
): ParseResult['quality'] {
  if (total === 0) return quality;
  if (ocred === 0 && quality === 'full') return 'degraded';
  return quality;
}

async function toUint8Array(
  input: Blob | ArrayBuffer | Uint8Array | File,
): Promise<Uint8Array> {
  if (input instanceof Uint8Array) return new Uint8Array(input);
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  const buf = await input.arrayBuffer();
  return new Uint8Array(buf);
}
