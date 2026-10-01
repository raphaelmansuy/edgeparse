/** SHA-256 hex digest of an ArrayBuffer (Web Crypto). */
export async function sha256Hex(data: ArrayBuffer | Uint8Array): Promise<string> {
  // Copy into a plain ArrayBuffer — Node/jsdom SubtleCrypto rejects
  // SharedArrayBuffer views and some sliced buffers from Response bodies.
  const src = data instanceof Uint8Array ? data : new Uint8Array(data);
  const copy = new Uint8Array(src.byteLength);
  copy.set(src);
  const hash = await crypto.subtle.digest('SHA-256', copy.buffer);
  return [...new Uint8Array(hash)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** True when the manifest still has an all-zero placeholder (publish guard). */
export function isPlaceholderHash(sha256: string): boolean {
  return /^0+$/.test(sha256);
}
