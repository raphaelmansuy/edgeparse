/** SHA-256 hex digest of an ArrayBuffer or Uint8Array. */

function toHex(bytes: ArrayBuffer | Uint8Array): string {
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  return [...u8].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/**
 * Prefer Node `crypto` when available (Vitest/jsdom SubtleCrypto is unreliable
 * with ArrayBuffer views). Fall back to Web Crypto in browsers.
 */
export async function sha256Hex(data: ArrayBuffer | Uint8Array): Promise<string> {
  const src = data instanceof Uint8Array ? data : new Uint8Array(data);

  const isNode =
    typeof process !== 'undefined' &&
    typeof process.versions === 'object' &&
    typeof process.versions.node === 'string';

  if (isNode) {
    const { createHash } = await import('node:crypto');
    return createHash('sha256').update(Buffer.from(src)).digest('hex');
  }

  const copy = new Uint8Array(src.byteLength);
  copy.set(src);
  const hash = await crypto.subtle.digest('SHA-256', copy);
  return toHex(hash);
}

/** True when the manifest still has an all-zero placeholder (publish guard). */
export function isPlaceholderHash(sha256: string): boolean {
  return /^0+$/.test(sha256);
}
