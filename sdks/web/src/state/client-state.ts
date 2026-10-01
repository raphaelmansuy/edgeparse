import type {
  Capabilities,
  ClientEventMap,
  EngineState,
  ErrorCode,
  JobProgress,
  JobStateName,
  ModelProgress,
  ModelStateName,
} from '../types.js';
import { emptySnapshot, StateStore } from './store.js';

type Handler<K extends keyof ClientEventMap> = (event: ClientEventMap[K]) => void;

/** Typed event bus + snapshot projection for engine / models / jobs. */
export class ClientState {
  readonly store = new StateStore(emptySnapshot());
  private handlers: { [K in keyof ClientEventMap]?: Set<Handler<K>> } = {};

  on<K extends keyof ClientEventMap>(type: K, handler: Handler<K>): () => void {
    const bucket = this.handlers[type] as Set<Handler<K>> | undefined;
    const set = bucket ?? new Set<Handler<K>>();
    if (!bucket) {
      (this.handlers as Record<string, Set<Handler<K>>>)[type as string] = set;
    }
    set.add(handler);
    return () => set.delete(handler);
  }

  emit<K extends keyof ClientEventMap>(type: K, event: ClientEventMap[K]): void {
    const set = this.handlers[type] as Set<Handler<K>> | undefined;
    if (!set) return;
    for (const h of set) h(event);
  }

  setCapabilities(caps: Capabilities): void {
    this.store.update((s) => {
      s.capabilities = caps;
    });
  }

  setOnline(online: boolean): void {
    this.store.update((s) => {
      s.online = online;
    });
    this.emit('offline', { online });
  }

  setEngine(state: EngineState, error?: string): void {
    this.store.update((s) => {
      s.engine = { state, error };
    });
    this.emit('engine:state', { state, error });
  }

  upsertModel(progress: ModelProgress): void {
    this.store.update((s) => {
      s.models[progress.id] = progress;
    });
    this.emit('model:progress', progress);
    if (progress.state === 'ready') {
      this.emit('model:ready', { id: progress.id });
    }
    if (progress.state === 'failed' && progress.error) {
      this.emit('model:failed', {
        id: progress.id,
        code: progress.error.code,
        message: progress.error.message,
      });
    }
  }

  ensureModel(id: string): ModelProgress {
    const existing = this.store.getSnapshot().models[id];
    if (existing) return existing;
    const fresh: ModelProgress = {
      id,
      state: 'absent',
      loaded: 0,
      total: 0,
      bytesPerSec: 0,
      etaSec: null,
      attempt: 0,
      source: null,
    };
    this.upsertModel(fresh);
    return fresh;
  }

  setModelState(
    id: string,
    state: ModelStateName,
    patch: Partial<ModelProgress> = {},
  ): ModelProgress {
    const prev = this.ensureModel(id);
    const next: ModelProgress = { ...prev, ...patch, id, state };
    this.upsertModel(next);
    return next;
  }

  upsertJob(progress: JobProgress): void {
    this.store.update((s) => {
      s.jobs[progress.jobId] = progress;
    });
    this.emit('job:progress', progress);
  }

  patchJob(
    jobId: string,
    state: JobStateName,
    patch: Partial<JobProgress>,
  ): JobProgress {
    const prev = this.store.getSnapshot().jobs[jobId] ?? {
      jobId,
      state: 'queued' as JobStateName,
      fraction: 0,
      label: '',
      ocrDone: 0,
      ocrTotal: 0,
      warnings: [],
    };
    const next: JobProgress = {
      ...prev,
      ...patch,
      jobId,
      state,
      fraction: Math.max(prev.fraction, patch.fraction ?? prev.fraction),
    };
    this.upsertJob(next);
    return next;
  }

  failJob(jobId: string, code: ErrorCode, message: string): void {
    this.patchJob(jobId, code === 'ABORTED' ? 'aborted' : 'failed', {
      label: message,
    });
    this.emit('job:failed', { jobId, code, message });
  }
}
