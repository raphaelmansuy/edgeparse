/** Closed set of SDK error codes. */
export type ErrorCode =
  | 'WASM_UNSUPPORTED'
  | 'PDF_ENCRYPTED'
  | 'PDF_INVALID'
  | 'MODEL_DOWNLOAD_FAILED'
  | 'MODEL_INTEGRITY_FAILED'
  | 'QUOTA_EXCEEDED'
  | 'OFFLINE'
  | 'ABORTED'
  | 'OCR_FAILED'
  | 'UNKNOWN';

export class EdgeParseError extends Error {
  readonly code: ErrorCode;
  readonly cause?: unknown;

  constructor(code: ErrorCode, message: string, cause?: unknown) {
    super(message);
    this.name = 'EdgeParseError';
    this.code = code;
    this.cause = cause;
  }
}

export type EngineState = 'idle' | 'loading-wasm' | 'ready' | 'unsupported' | 'error';

export type ModelStateName =
  | 'absent'
  | 'checking'
  | 'downloading'
  | 'verifying'
  | 'ready'
  | 'failed';

export type JobStateName =
  | 'queued'
  | 'opening'
  | 'planning'
  | 'ocr'
  | 'assembling'
  | 'done'
  | 'failed'
  | 'aborted';

export type ModelTier = 'tiny' | 'small' | 'medium';
export type ModelsPolicy = 'lazy' | 'preload' | 'manual' | 'off';
export type ResultQuality = 'full' | 'degraded' | 'skipped';

export interface ModelManifestEntry {
  id: string;
  tier: ModelTier;
  kind: 'detection' | 'recognition' | 'dictionary' | 'other';
  urls: string[];
  bytes: number;
  sha256: string;
  license: string;
  minCapability: 'simd' | 'webgpu' | 'none';
}

export interface ModelManifest {
  version: string;
  defaultTier: ModelTier;
  models: ModelManifestEntry[];
}

export interface ModelProgress {
  id: string;
  state: ModelStateName;
  loaded: number;
  total: number;
  bytesPerSec: number;
  etaSec: number | null;
  attempt: number;
  source: string | null;
  error?: { code: ErrorCode; message: string };
}

export interface JobProgress {
  jobId: string;
  state: JobStateName;
  fraction: number;
  label: string;
  ocrDone: number;
  ocrTotal: number;
  warnings: string[];
}

export interface Capabilities {
  wasm: boolean;
  simd: boolean;
  threads: boolean;
  crossOriginIsolated: boolean;
  webgpu: boolean;
  deviceMemoryGb: number | null;
  opfs: boolean;
  webLocks: boolean;
  saveData: boolean;
  offline: boolean;
}

export interface ParseOptions {
  format?: 'markdown' | 'json' | 'html' | 'text' | 'all';
  /** When true with format json/markdown, request all format strings in one finish. */
  wantAllFormats?: boolean;
  tableMethod?: 'default' | 'cluster';
  readingOrder?: 'auto' | 'off';
  pages?: string;
  fileName?: string;
  signal?: AbortSignal;
  backendMarkdown?: string | null;
}

export interface ResultMeta {
  wasmVersion: string;
  models: Array<{ id: string; version: string; sha256: string }>;
  timingsMs: Record<string, number>;
  imagesTotal: number;
  imagesOcred: number;
  cacheHits: number;
  backend: 'webgpu' | 'wasm-simd' | 'wasm' | 'node' | 'none';
  tier: ModelTier | 'off';
}

export interface ParseResult {
  markdown?: string;
  html?: string;
  text?: string;
  json?: string;
  document?: unknown;
  warnings: string[];
  quality: ResultQuality;
  meta: ResultMeta;
}

export interface EdgeParseOptions {
  models?: ModelsPolicy;
  ocr?: ModelTier | 'off';
  wasmUrl?: string;
  manifest?: ModelManifest;
  onBeforeDownload?: (model: ModelManifestEntry) => boolean | Promise<boolean>;
  onTelemetry?: (event: string, data: Record<string, unknown>) => void;
  signal?: AbortSignal;
  /** Override worker URLs (tests / custom bundlers). */
  parseWorkerUrl?: string | URL;
  ocrWorkerUrl?: string | URL;
}

export type Snapshot = {
  engine: { state: EngineState; error?: string };
  models: Record<string, ModelProgress>;
  jobs: Record<string, JobProgress>;
  capabilities: Capabilities | null;
  online: boolean;
};

export type ClientEventMap = {
  'engine:state': { state: EngineState; error?: string };
  'model:progress': ModelProgress;
  'model:ready': { id: string };
  'model:failed': { id: string; code: ErrorCode; message: string };
  'job:progress': JobProgress;
  'job:done': { jobId: string; result: ParseResult };
  'job:failed': { jobId: string; code: ErrorCode; message: string };
  offline: { online: boolean };
};
