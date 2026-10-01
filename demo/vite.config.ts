import { defineConfig } from 'vite';
import path from 'node:path';
import fs from 'node:fs';

const webDist = path.resolve(__dirname, '../sdks/web/dist/index.js');
const webSrc = path.resolve(__dirname, '../sdks/web/src/index.ts');

export default defineConfig(({ command }) => {
  // Dev: source for HMR. Production: built dist so worker defaults resolve to .js
  // (Vite otherwise rewrites ./workers/parse-worker.js → a .ts static asset).
  const useWebSrc = command === 'serve' || !fs.existsSync(webDist);

  return {
    base: process.env.DEMO_BASE_PATH || '/',
    resolve: {
      alias: {
        'edgeparse-wasm': path.resolve(__dirname, '../crates/edgeparse-wasm/pkg'),
        '@edgeparse/web': useWebSrc ? webSrc : webDist,
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
  };
});
