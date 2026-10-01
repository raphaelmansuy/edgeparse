/** SHA-256 hex digest of an ArrayBuffer (Web Crypto). */
export async function sha256Hex(data: ArrayBuffer | Uint8Array): Promise<string> {
  const buf =
    data instanceof Uint8Array
      ? data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength)
      : data;
  const hash = await crypto.subtle.digest('SHA-256', buf as ArrayBuffer);
  return [...new Uint8Array(hash)].map((b) => b.toString(16).padStart(2, '0')).join('');
}

/** True when the manifest still has an all-zero placeholder (publish guard). */
export function isPlaceholderHash(sha256: string): boolean {
  return /^0+$/.test(sha256);
}
