export { EdgeParse, ParseJob } from './client.js';
export { probeCapabilities, pickBackend, pickDefaultTier } from './capabilities.js';
export { ClientState } from './state/client-state.js';
export { StateStore, ProgressTracker, JOB_WEIGHTS, emptySnapshot } from './state/store.js';
export {
  ModelManager,
  getDefaultManifest,
  sha256Hex,
  isPlaceholderHash,
  loadCached,
  storeModel,
} from './models/index.js';
export { OnnxOcrBackend, NullOcrBackend } from './ocr/onnx-backend.js';
export { PpocrPipeline } from './ocr/ppocr/pipeline.js';
export { ctcGreedyDecode, parseDictionary } from './ocr/ppocr/ctc-decode.js';
export { OcrResultCache } from './ocr/types.js';
export type { OcrBackend, OcrWord, OcrRequest } from './ocr/types.js';
export { EdgeParseError } from './types.js';
export type {
  Capabilities,
  ClientEventMap,
  EdgeParseOptions,
  EngineState,
  ErrorCode,
  JobProgress,
  JobStateName,
  ModelManifest,
  ModelManifestEntry,
  ModelProgress,
  ModelStateName,
  ModelTier,
  ModelsPolicy,
  ParseOptions,
  ParseResult,
  ResultMeta,
  ResultQuality,
  Snapshot,
} from './types.js';
