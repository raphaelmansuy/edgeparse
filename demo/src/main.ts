import './style.css';
import { mountApp } from './components/app-shell';
import { store } from './state';
import { ensureWasm, parsePdf } from './services/wasm-bridge';

// Mount the application shell
const root = document.querySelector<HTMLDivElement>('#app');
if (!root) throw new Error('Missing #app element');
mountApp(root);

// Pre-warm WASM so it's ready when the user uploads their first PDF
ensureWasm();

let parseGeneration = 0;

async function runParse(): Promise<void> {
  const bytes = store.get('pdfBytes');
  if (!bytes) return;
  const gen = ++parseGeneration;
  try {
    const { document, cache } = await parsePdf(bytes);
    // Ignore stale results if a newer parse was started (e.g. OCR toggle).
    if (gen !== parseGeneration) return;
    store.patch({
      parsedDocument: document,
      formatCache: cache,
      parseEpoch: store.get('parseEpoch') + 1,
      outputText: cache[store.get('outputFormat')] ?? '',
    });
  } catch (err: unknown) {
    if (gen !== parseGeneration) return;
    store.set('errorMessage', `Parse error: ${err}`);
  }
}

// Parse PDF whenever pdfBytes changes (user upload or drag-drop)
store.subscribe('pdfBytes', () => {
  void runParse();
});

// Re-parse when OCR is toggled (same bytes, different pipeline)
store.subscribe('enableOcr', () => {
  if (store.get('pdfBytes')) {
    void runParse();
  }
});
