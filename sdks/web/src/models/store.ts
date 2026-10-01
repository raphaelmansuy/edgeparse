import type { ModelManifestEntry } from '../types.js';
import { isPlaceholderHash, sha256Hex } from './hash.js';

export interface StoredModel {
  bytes: Uint8Array;
  sha256: string;
  size: number;
  source: 'opfs' | 'cache' | 'memory';
}

interface DoneMarker {
  size: number;
  sha256: string;
}

const CACHE_NAME = 'edgeparse-models-v1';
const memoryFallback = new Map<string, StoredModel>();

async function opfsRoot(): Promise<FileSystemDirectoryHandle | null> {
  try {
    if (!('storage' in navigator) || !('getDirectory' in navigator.storage)) {
      return null;
    }
    return await navigator.storage.getDirectory();
  } catch {
    return null;
  }
}

async function readOpfs(id: string): Promise<StoredModel | null> {
  const root = await opfsRoot();
  if (!root) return null;
  try {
    const dir = await root.getDirectoryHandle('edgeparse-models');
    const doneHandle = await dir.getFileHandle(`${id}.done`);
    const doneFile = await doneHandle.getFile();
    const marker = JSON.parse(await doneFile.text()) as DoneMarker;
    const dataHandle = await dir.getFileHandle(id);
    const dataFile = await dataHandle.getFile();
    if (dataFile.size !== marker.size) return null;
    const buf = new Uint8Array(await dataFile.arrayBuffer());
    return { bytes: buf, sha256: marker.sha256, size: marker.size, source: 'opfs' };
  } catch {
    return null;
  }
}

async function writeOpfs(
  id: string,
  bytes: Uint8Array,
  sha256: string,
): Promise<boolean> {
  const root = await opfsRoot();
  if (!root) return false;
  try {
    const dir = await root.getDirectoryHandle('edgeparse-models', { create: true });
    const dataHandle = await dir.getFileHandle(id, { create: true });
    const writable = await dataHandle.createWritable();
    await writable.write(bytes as unknown as Blob);
    await writable.close();
    const doneHandle = await dir.getFileHandle(`${id}.done`, { create: true });
    const doneWritable = await doneHandle.createWritable();
    const marker: DoneMarker = { size: bytes.byteLength, sha256 };
    await doneWritable.write(JSON.stringify(marker));
    await doneWritable.close();
    return true;
  } catch {
    return false;
  }
}

async function readCache(id: string): Promise<StoredModel | null> {
  if (typeof caches === 'undefined') return null;
  try {
    const cache = await caches.open(CACHE_NAME);
    const res = await cache.match(`model://${id}`);
    if (!res) return null;
    const sha256 = res.headers.get('x-edgeparse-sha256') ?? '';
    const buf = new Uint8Array(await res.arrayBuffer());
    return { bytes: buf, sha256, size: buf.byteLength, source: 'cache' };
  } catch {
    return null;
  }
}

async function writeCache(id: string, bytes: Uint8Array, sha256: string): Promise<boolean> {
  if (typeof caches === 'undefined') return false;
  try {
    const cache = await caches.open(CACHE_NAME);
    const headers = new Headers({
      'content-type': 'application/octet-stream',
      'x-edgeparse-sha256': sha256,
      'x-edgeparse-size': String(bytes.byteLength),
    });
    await cache.put(
      `model://${id}`,
      new Response(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer, {
        headers,
      }),
    );
    return true;
  } catch {
    return false;
  }
}

export async function loadCached(id: string): Promise<StoredModel | null> {
  const fromOpfs = await readOpfs(id);
  if (fromOpfs) return fromOpfs;
  const fromCache = await readCache(id);
  if (fromCache) return fromCache;
  return memoryFallback.get(id) ?? null;
}

export async function storeModel(
  entry: ModelManifestEntry,
  bytes: Uint8Array,
): Promise<StoredModel> {
  const digest = await sha256Hex(bytes);
  if (isPlaceholderHash(entry.sha256)) {
    throw new Error(
      `model ${entry.id} has placeholder sha256 — refuse to store unverified models`,
    );
  }
  if (digest !== entry.sha256) {
    throw new Error(`sha256 mismatch for ${entry.id}: got ${digest}`);
  }
  const stored: StoredModel = {
    bytes,
    sha256: digest,
    size: bytes.byteLength,
    source: 'memory',
  };
  if (await writeOpfs(entry.id, bytes, digest)) {
    stored.source = 'opfs';
  } else if (await writeCache(entry.id, bytes, digest)) {
    stored.source = 'cache';
  } else {
    memoryFallback.set(entry.id, stored);
  }
  return stored;
}

export async function estimateQuota(): Promise<{ usage: number; quota: number } | null> {
  try {
    if (!navigator.storage?.estimate) return null;
    const est = await navigator.storage.estimate();
    return { usage: est.usage ?? 0, quota: est.quota ?? 0 };
  } catch {
    return null;
  }
}

export async function requestPersist(): Promise<boolean> {
  try {
    return (await navigator.storage?.persist?.()) ?? false;
  } catch {
    return false;
  }
}
