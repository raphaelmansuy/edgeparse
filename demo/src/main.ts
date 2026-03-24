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

// Parse PDF whenever pdfBytes changes (user upload or drag-drop)
store.subscribe('pdfBytes', async () => {
  const bytes = store.get('pdfBytes');
  if (!bytes) return;
  try {
    const { document, cache } = await parsePdf(bytes);
    store.set('parsedDocument', document);
    store.set('formatCache', cache);
    store.set('outputText', cache[store.get('outputFormat')]);
  } catch (err: unknown) {
    store.set('errorMessage', `Parse error: ${err}`);
  }
});
