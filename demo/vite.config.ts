import { defineConfig } from 'vite';
import path from 'node:path';

export default defineConfig({
  base: process.env.DEMO_BASE_PATH || '/',
  resolve: {
    alias: {
      'edgeparse-wasm': path.resolve(__dirname, '../crates/edgeparse-wasm/pkg'),
      '@edgeparse/web': path.resolve(__dirname, '../sdks/web/src/index.ts'),
    },
  },
  optimizeDeps: {
    exclude: ['@edgeparse/edgeparse-wasm', 'edgeparse-wasm', '@edgeparse/web'],
  },
  worker: {
    format: 'es',
  },
  build: {
    target: 'esnext',
  },
  server: {
    fs: {
      allow: ['..'],
    },
  },
  assetsInclude: ['**/*.wasm', '**/*.ort'],
});
