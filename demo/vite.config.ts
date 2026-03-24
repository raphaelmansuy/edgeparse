import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    exclude: ['@edgeparse/edgeparse-wasm'],
  },
  build: {
    target: 'esnext',
  },
  server: {
    headers: {
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
    },
    fs: {
      allow: ['..'],
    },
  },
});
