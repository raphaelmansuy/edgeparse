import type { ModelManifest, ModelManifestEntry, ModelTier } from '../types.js';
import { EdgeParseError } from '../types.js';
import { isPlaceholderHash, sha256Hex } from './hash.js';
import {
  estimateQuota,
  loadCached,
  requestPersist,
  storeModel,
  type StoredModel,
} from './store.js';
import type { ClientState } from '../state/client-state.js';

export type BeforeDownload = (model: ModelManifestEntry) => boolean | Promise<boolean>;

export interface ModelManagerOptions {
  manifest: ModelManifest;
  state: ClientState;
  onBeforeDownload?: BeforeDownload;
  fetchImpl?: typeof fetch;
}

const LOCK_PREFIX = 'edgeparse-model:';
const CHANNEL = 'edgeparse-models';

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

function jitter(base: number): number {
  return base + Math.random() * base * 0.4;
}

export class ModelManager {
  private readonly manifest: ModelManifest;
  private readonly state: ClientState;
  private readonly onBeforeDownload?: BeforeDownload;
  private readonly fetchImpl: typeof fetch;
  private readonly channel: BroadcastChannel | null;
  private readonly inflight = new Map<string, Promise<StoredModel>>();

  constructor(opts: ModelManagerOptions) {
    this.manifest = opts.manifest;
    this.state = opts.state;
    this.onBeforeDownload = opts.onBeforeDownload;
    this.fetchImpl = opts.fetchImpl ?? fetch.bind(globalThis);
    this.channel =
      typeof BroadcastChannel !== 'undefined'
        ? new BroadcastChannel(CHANNEL)
        : null;
    this.channel?.addEventListener('message', (ev) => {
      const data = ev.data as { type: string; progress?: import('../types.js').ModelProgress };
      if (data.type === 'progress' && data.progress) {
        this.state.upsertModel(data.progress);
      }
    });
  }

  getManifest(): ModelManifest {
    return this.manifest;
  }

  modelsForTier(tier: ModelTier): ModelManifestEntry[] {
    return this.manifest.models.filter((m) => m.tier === tier);
  }

  async ensureTier(tier: ModelTier, signal?: AbortSignal): Promise<StoredModel[]> {
    const entries = this.modelsForTier(tier);
    const out: StoredModel[] = [];
    for (const entry of entries) {
      out.push(await this.ensure(entry.id, signal));
    }
    return out;
  }

  async ensure(id: string, signal?: AbortSignal): Promise<StoredModel> {
    const existing = this.inflight.get(id);
    if (existing) return existing;
    const run = this.ensureLocked(id, signal);
    this.inflight.set(id, run);
    try {
      return await run;
    } finally {
      this.inflight.delete(id);
    }
  }

  private async ensureLocked(id: string, signal?: AbortSignal): Promise<StoredModel> {
    const entry = this.manifest.models.find((m) => m.id === id);
    if (!entry) {
      throw new EdgeParseError('MODEL_DOWNLOAD_FAILED', `Unknown model id: ${id}`);
    }

    const run = async (): Promise<StoredModel> => {
      this.state.setModelState(id, 'checking', { total: entry.bytes, loaded: 0 });
      const cached = await loadCached(id);
      if (cached) {
        const ok =
          cached.sha256 === entry.sha256 ||
          (await sha256Hex(cached.bytes)) === entry.sha256;
        if (ok && !isPlaceholderHash(entry.sha256)) {
          this.state.setModelState(id, 'ready', {
            loaded: cached.size,
            total: cached.size,
            source: cached.source,
          });
          return cached;
        }
      }

      if (typeof navigator !== 'undefined' && !navigator.onLine) {
        this.state.setModelState(id, 'failed', {
          error: { code: 'OFFLINE', message: 'Offline and model not cached' },
        });
        throw new EdgeParseError('OFFLINE', `Model ${id} not cached and offline`);
      }

      if (this.onBeforeDownload) {
        const ok = await this.onBeforeDownload(entry);
        if (!ok) {
          throw new EdgeParseError(
            'ABORTED',
            `User declined download of ${id}`,
          );
        }
      }
      if (
        typeof navigator !== 'undefined' &&
        'connection' in navigator &&
        (navigator as Navigator & { connection?: { saveData?: boolean } }).connection
          ?.saveData
      ) {
        const ok = this.onBeforeDownload
          ? await this.onBeforeDownload(entry)
          : false;
        if (!ok) {
          throw new EdgeParseError('ABORTED', `saveData: skipped download of ${id}`);
        }
      }

      const quota = await estimateQuota();
      if (quota && quota.quota > 0 && quota.usage + entry.bytes > quota.quota) {
        this.state.setModelState(id, 'failed', {
          error: { code: 'QUOTA_EXCEEDED', message: 'Storage quota exceeded' },
        });
        throw new EdgeParseError('QUOTA_EXCEEDED', `Quota exceeded for ${id}`);
      }
      await requestPersist();

      const bytes = await this.downloadWithFailover(entry, signal);
      this.state.setModelState(id, 'verifying', {
        loaded: bytes.byteLength,
        total: bytes.byteLength,
      });
      try {
        const stored = await storeModel(entry, bytes);
        this.state.setModelState(id, 'ready', {
          loaded: stored.size,
          total: stored.size,
          source: stored.source,
        });
        return stored;
      } catch (err) {
        this.state.setModelState(id, 'failed', {
          error: {
            code: 'MODEL_INTEGRITY_FAILED',
            message: err instanceof Error ? err.message : String(err),
          },
        });
        throw new EdgeParseError(
          'MODEL_INTEGRITY_FAILED',
          err instanceof Error ? err.message : String(err),
          err,
        );
      }
    };

    if (typeof navigator !== 'undefined' && navigator.locks?.request) {
      return navigator.locks.request(`${LOCK_PREFIX}${id}`, run);
    }
    return run();
  }

  private async downloadWithFailover(
    entry: ModelManifestEntry,
    signal?: AbortSignal,
  ): Promise<Uint8Array> {
    let lastErr: unknown;
    for (let attempt = 1; attempt <= 4; attempt++) {
      for (const url of entry.urls) {
        try {
          const bytes = await this.downloadOne(entry, url, attempt, signal);
          return bytes;
        } catch (err) {
          lastErr = err;
          if (signal?.aborted) {
            throw new EdgeParseError('ABORTED', 'Download aborted', err);
          }
        }
      }
      await sleep(jitter(200 * 2 ** (attempt - 1)));
    }
    this.state.setModelState(entry.id, 'failed', {
      error: {
        code: 'MODEL_DOWNLOAD_FAILED',
        message: lastErr instanceof Error ? lastErr.message : String(lastErr),
      },
    });
    throw new EdgeParseError(
      'MODEL_DOWNLOAD_FAILED',
      `Failed to download ${entry.id}`,
      lastErr,
    );
  }

  private async downloadOne(
    entry: ModelManifestEntry,
    url: string,
    attempt: number,
    signal?: AbortSignal,
  ): Promise<Uint8Array> {
    const started = performance.now();
    let loaded = 0;
    this.state.setModelState(entry.id, 'downloading', {
      attempt,
      source: url,
      loaded: 0,
      total: entry.bytes,
      bytesPerSec: 0,
      etaSec: null,
    });

    const res = await this.fetchImpl(url, { signal });
    if (!res.ok || !res.body) {
      throw new Error(`HTTP ${res.status} for ${url}`);
    }
    const total = Number(res.headers.get('content-length') ?? entry.bytes) || entry.bytes;
    const reader = res.body.getReader();
    const chunks: Uint8Array[] = [];
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      chunks.push(value);
      loaded += value.byteLength;
      const elapsed = (performance.now() - started) / 1000;
      const bps = elapsed > 0 ? loaded / elapsed : 0;
      const eta = bps > 0 && total > loaded ? (total - loaded) / bps : null;
      const progress = this.state.setModelState(entry.id, 'downloading', {
        attempt,
        source: url,
        loaded,
        total,
        bytesPerSec: bps,
        etaSec: eta,
      });
      this.channel?.postMessage({ type: 'progress', progress });
    }
    const out = new Uint8Array(loaded);
    let offset = 0;
    for (const c of chunks) {
      out.set(c, offset);
      offset += c.byteLength;
    }
    return out;
  }
}
