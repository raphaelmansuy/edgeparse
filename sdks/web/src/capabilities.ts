import type { Capabilities } from './types.js';

function hasSimd(): boolean {
  try {
    // V128 const opcode probe (WebAssembly SIMD).
    return WebAssembly.validate(
      new Uint8Array([
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x05, 0x01, 0x60,
        0x00, 0x01, 0x7b, 0x03, 0x02, 0x01, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00,
        0x41, 0x00, 0xfd, 0x0f, 0x26, 0x0b,
      ]),
    );
  } catch {
    return false;
  }
}

export async function probeCapabilities(): Promise<Capabilities> {
  const wasm = typeof WebAssembly !== 'undefined';
  const simd = wasm && hasSimd();
  const crossOriginIsolated =
    typeof globalThis !== 'undefined' &&
    Boolean((globalThis as { crossOriginIsolated?: boolean }).crossOriginIsolated);
  const threads = crossOriginIsolated && typeof SharedArrayBuffer !== 'undefined';
  let webgpu = false;
  try {
    webgpu = Boolean(
      typeof navigator !== 'undefined' &&
        'gpu' in navigator &&
        (await (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown> } }).gpu
          ?.requestAdapter()),
    );
  } catch {
    webgpu = false;
  }
  const deviceMemoryGb =
    typeof navigator !== 'undefined' && 'deviceMemory' in navigator
      ? Number((navigator as Navigator & { deviceMemory?: number }).deviceMemory) || null
      : null;
  let opfs = false;
  try {
    opfs = Boolean(
      typeof navigator !== 'undefined' &&
        navigator.storage &&
        'getDirectory' in navigator.storage,
    );
  } catch {
    opfs = false;
  }
  const webLocks =
    typeof navigator !== 'undefined' && Boolean(navigator.locks?.request);
  const saveData = Boolean(
    typeof navigator !== 'undefined' &&
      (navigator as Navigator & { connection?: { saveData?: boolean } }).connection
        ?.saveData,
  );
  const offline = typeof navigator !== 'undefined' ? !navigator.onLine : false;

  return {
    wasm,
    simd,
    threads,
    crossOriginIsolated,
    webgpu,
    deviceMemoryGb,
    opfs,
    webLocks,
    saveData,
    offline,
  };
}

export function pickBackend(
  caps: Capabilities,
): 'webgpu' | 'wasm-simd' | 'wasm' | 'none' {
  if (!caps.wasm) return 'none';
  if (caps.webgpu) return 'webgpu';
  if (caps.simd) return 'wasm-simd';
  return 'wasm';
}

export function pickDefaultTier(
  caps: Capabilities,
): 'tiny' | 'small' | 'medium' {
  if (caps.saveData || (caps.deviceMemoryGb !== null && caps.deviceMemoryGb <= 2)) {
    return 'tiny';
  }
  if (caps.webgpu && (caps.deviceMemoryGb === null || caps.deviceMemoryGb >= 4)) {
    return 'small';
  }
  return 'small';
}
