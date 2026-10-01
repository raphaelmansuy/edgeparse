import { describe, expect, it, vi } from 'vitest';
import { ProgressTracker, JOB_WEIGHTS } from '../src/state/store.js';
import { ClientState } from '../src/state/client-state.js';
import { ModelManager } from '../src/models/manager.js';
import { sha256Hex, isPlaceholderHash } from '../src/models/hash.js';
import type { ModelManifest } from '../src/types.js';

describe('ProgressTracker', () => {
  it('is monotonic', () => {
    const t = new ProgressTracker();
    expect(t.set(0.2)).toBeCloseTo(0.2);
    expect(t.set(0.1)).toBeCloseTo(0.2);
    expect(t.set(0.5)).toBeCloseTo(0.5);
    expect(t.advance(JOB_WEIGHTS.ocr, 0.5)).toBeGreaterThan(0.5);
  });
});

describe('ClientState', () => {
  it('projects engine and model events', () => {
    const s = new ClientState();
    const events: string[] = [];
    s.on('engine:state', (e) => events.push(e.state));
    s.on('model:ready', (e) => events.push(`ready:${e.id}`));
    s.setEngine('loading-wasm');
    s.setModelState('m1', 'ready', { loaded: 10, total: 10 });
    expect(s.store.getSnapshot().engine.state).toBe('loading-wasm');
    expect(s.store.getSnapshot().models.m1?.state).toBe('ready');
    expect(events).toContain('loading-wasm');
    expect(events).toContain('ready:m1');
  });

  it('job fraction never decreases', () => {
    const s = new ClientState();
    s.patchJob('j1', 'planning', { fraction: 0.4, label: 'a' });
    s.patchJob('j1', 'ocr', { fraction: 0.2, label: 'b' });
    expect(s.store.getSnapshot().jobs.j1?.fraction).toBe(0.4);
  });
});

describe('sha256', () => {
  it('hashes known vector', async () => {
    const hex = await sha256Hex(new TextEncoder().encode('abc'));
    expect(hex).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
    );
  });

  it('detects placeholder', () => {
    expect(isPlaceholderHash('0000')).toBe(true);
    expect(isPlaceholderHash('abc')).toBe(false);
  });
});

describe('ModelManager', () => {
  const manifest: ModelManifest = {
    version: 'test',
    defaultTier: 'tiny',
    models: [
      {
        id: 'test-model',
        tier: 'tiny',
        kind: 'detection',
        urls: ['https://example.test/a.bin', 'https://example.test/b.bin'],
        bytes: 4,
        sha256: '9f64a747e1b97f131fabb6b447296c9b6f0201e79fb3c5356e6c77e89b6a806a',
        license: 'Apache-2.0',
        minCapability: 'none',
      },
    ],
  };

  it('downloads with mirror failover and verifies', async () => {
    const state = new ClientState();
    let calls = 0;
    const fetchImpl = vi.fn(async (url: string) => {
      calls += 1;
      if (String(url).includes('/a.bin') && calls === 1) {
        return new Response(null, { status: 500 });
      }
      const body = new Uint8Array([1, 2, 3, 4]);
      return new Response(body, {
        status: 200,
        headers: { 'content-length': '4' },
      });
    }) as unknown as typeof fetch;

    const mm = new ModelManager({
      manifest,
      state,
      fetchImpl,
      onBeforeDownload: async () => true,
    });
    const stored = await mm.ensure('test-model');
    expect(stored.size).toBe(4);
    expect(state.store.getSnapshot().models['test-model']?.state).toBe('ready');
  });

  it('respects decline consent as abort', async () => {
    const state = new ClientState();
    const mm = new ModelManager({
      manifest: {
        ...manifest,
        models: [{ ...manifest.models[0]!, id: 'declined-model' }],
      },
      state,
      onBeforeDownload: async () => false,
      fetchImpl: vi.fn() as unknown as typeof fetch,
    });
    await expect(mm.ensure('declined-model')).rejects.toMatchObject({ code: 'ABORTED' });
  });

  it('emits MODEL_INTEGRITY_FAILED on hash mismatch', async () => {
    const badManifest: ModelManifest = {
      ...manifest,
      models: [
        {
          ...manifest.models[0]!,
          id: 'bad-hash',
          sha256: 'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
        },
      ],
    };
    const state = new ClientState();
    const fetchImpl = vi.fn(async () => {
      return new Response(new Uint8Array([9, 9, 9, 9]), {
        status: 200,
        headers: { 'content-length': '4' },
      });
    }) as unknown as typeof fetch;
    const mm = new ModelManager({
      manifest: badManifest,
      state,
      fetchImpl,
      onBeforeDownload: async () => true,
    });
    await expect(mm.ensure('bad-hash')).rejects.toMatchObject({
      code: 'MODEL_INTEGRITY_FAILED',
    });
  });
});
