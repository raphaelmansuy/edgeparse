import { defineConfig } from 'tsup';

export default defineConfig({
  entry: {
    index: 'src/index.ts',
    'models/index': 'src/models/index.ts',
    'workers/parse-worker': 'src/workers/parse-worker.ts',
    'workers/ocr-worker': 'src/workers/ocr-worker.ts',
    'node/index': 'src/node/index.ts',
    'node/ppocr': 'src/node/ppocr.ts',
  },
  format: ['esm'],
  dts: false,
  sourcemap: true,
  clean: true,
  splitting: false,
  treeshake: true,
  target: 'es2022',
  platform: 'browser',
  outDir: 'dist',
  external: [
    'edgeparse-wasm',
    'onnxruntime-web',
    'onnxruntime-node',
    'node:fs',
    'node:path',
    'node:crypto',
    'node:worker_threads',
    'node:module',
    'node:fs/promises',
    'node:url',
  ],
});
