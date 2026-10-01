import type { ModelManifest } from '../types.js';
import defaultManifest from '../../models/models.json';

export { ModelManager } from './manager.js';
export type { BeforeDownload, ModelManagerOptions } from './manager.js';
export { loadCached, storeModel, estimateQuota } from './store.js';
export { sha256Hex, isPlaceholderHash } from './hash.js';

export function getDefaultManifest(): ModelManifest {
  return defaultManifest as ModelManifest;
}
