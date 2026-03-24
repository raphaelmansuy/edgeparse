/**
 * OutputViewer — right pane.
 *
 * Performance design:
 *  • JSON / Text   → CodeMirror (virtualizes rows internally — always fast)
 *  • Markdown      → marked.parse() runs in render-worker (off-thread), then
 *                    DOMPurify sanitizes, result cached in renderCache
 *  • HTML          → DOMPurify only (no markdown conversion); deferred one rAF
 *                    so the loading overlay paints before the sync work runs
 *
 * renderCache (Map<format → sanitized HTML string>) is cleared when a new PDF
 * is loaded (outputText changes), so stale renders are never served.
 *
 * renderRevision guards against race conditions when the user switches tabs
 * quickly while an async render is in flight.
 */

import { el } from '../utils/dom';
import { store } from '../state';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { json } from '@codemirror/lang-json';
import { oneDark } from '@codemirror/theme-one-dark';
import DOMPurify from 'dompurify';
import type { OutputFormat } from '../types';

// ── Render worker (marked.parse off-thread) ───────────────────────────────────

type RenderWorkerMsg =
  | { id: string; ok: true;  html: string }
  | { id: string; ok: false; error: string };

const renderWorker = new Worker(
  new URL('../workers/render-worker.ts', import.meta.url),
  { type: 'module' },
);

const renderPending = new Map<
  string,
  { resolve: (h: string) => void; reject: (e: unknown) => void }
>();
let _rid = 0;

renderWorker.addEventListener('message', (e: MessageEvent<RenderWorkerMsg>) => {
  const msg = e.data;
  const p = renderPending.get(msg.id);
  if (!p) return;
  renderPending.delete(msg.id);
  if (msg.ok) p.resolve(msg.html);
  else p.reject(new Error(msg.error));
});

function markdownToHtml(markdown: string): Promise<string> {
  const id = String(_rid++);
  return new Promise((resolve, reject) => {
    renderPending.set(id, { resolve, reject });
    renderWorker.postMessage({ id, markdown });
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Yield to the browser so pending paints (e.g. loading overlay) flush first. */
function nextPaint(): Promise<void> {
  return new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r())));
}

/** Sanitize and inject HTML into a container using a <template> inert parse. */
function applyHtml(target: HTMLElement, html: string): void {
  const clean = DOMPurify.sanitize(html);
  const tmpl = document.createElement('template');
  tmpl.innerHTML = clean;
  target.replaceChildren(tmpl.content);
}

// ── Module-level state ────────────────────────────────────────────────────────

let editorView: EditorView | null = null;

// Sanitized HTML string cache — keyed by OutputFormat, cleared on new PDF.
const renderCache = new Map<OutputFormat, string>();

// Monotonically-increasing counter. Each async render captures the current
// value; if it has changed by the time the render completes the result is
// discarded (user switched tabs again).
let renderRevision = 0;

// True while an async render (worker + DOMPurify) is running.
let renderBusy = false;

// ── Component ─────────────────────────────────────────────────────────────────

export function createOutputViewer(): HTMLElement {
  const container = el('div', { className: 'output-viewer' });

  const codePane = el('div', { className: 'output-viewer__code' });
  const renderedPane = el('div', {
    className: 'output-viewer__rendered',
    style: 'display: none;',
  });

  const actions = el('div', {
    className: 'output-viewer__actions',
    innerHTML: `
      <button class="output-viewer__btn" data-action="copy"     aria-label="Copy output">Copy</button>
      <button class="output-viewer__btn" data-action="download" aria-label="Download output">Download</button>
    `,
  });

  // Shared loading overlay — covers both parse-busy and render-busy states.
  const loadingOverlay = el('div', {
    className: 'output-viewer__loading',
    ariaLive: 'polite',
    ariaLabel: 'Processing',
  });
  loadingOverlay.innerHTML = `
    <div class="loading-spinner" aria-hidden="true"></div>
    <p class="loading-text">Loading…</p>
  `;
  loadingOverlay.style.display = 'none';

  container.append(actions, codePane, renderedPane, loadingOverlay);

  // CodeMirror
  const cmState = EditorState.create({
    doc: '',
    extensions: [
      basicSetup,
      json(),
      EditorView.editable.of(false),
      EditorState.readOnly.of(true),
      EditorView.lineWrapping,
    ],
  });
  editorView = new EditorView({ state: cmState, parent: codePane });

  // ── Loading overlay management ──────────────────────────────────────────────

  function loadingMsg(): string {
    if (renderBusy) return 'Rendering…';
    const ws = store.get('wasmStatus');
    return ws === 'loading' ? 'Loading parser…' : 'Parsing PDF…';
  }

  function updateLoadingState(): void {
    const ws  = store.get('wasmStatus');
    const ps  = store.get('parseStatus');
    const has = !!store.get('pdfBytes');
    const parseBusy = ps === 'parsing' || (ws === 'loading' && has);
    const busy = parseBusy || renderBusy;
    loadingOverlay.style.display = busy ? '' : 'none';
    (loadingOverlay.querySelector('.loading-text') as HTMLElement).textContent =
      loadingMsg();
  }

  store.subscribe('wasmStatus',  updateLoadingState);
  store.subscribe('parseStatus', updateLoadingState);
  store.subscribe('pdfBytes',    updateLoadingState);
  updateLoadingState();

  // ── Content update ──────────────────────────────────────────────────────────

  async function updateContent(): Promise<void> {
    const text   = store.get('outputText');
    const format = store.get('outputFormat');

    const isCode = format === 'json' || format === 'text';

    // Switch CodeMirror / rendered pane visibility
    const codePaneEl = editorView?.dom.parentElement;
    if (codePaneEl) codePaneEl.style.display = isCode ? '' : 'none';
    renderedPane.style.display = isCode ? 'none' : '';

    if (isCode) {
      // Cancel any in-flight render so its finally-block won't re-show overlay.
      renderBusy = false;
      renderRevision++;
      updateLoadingState();
      editorView?.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: text },
      });
      return;
    }

    // ── Rendered view (markdown | html) ────────────────────────────────────

    const cached = renderCache.get(format);
    if (cached !== undefined) {
      // Instant: re-use cached sanitized HTML string
      applyHtml(renderedPane, cached);
      return;
    }

    // Show loading overlay immediately, then do the heavy work.
    const rev = ++renderRevision;
    renderBusy = true;
    updateLoadingState();

    try {
      let rawHtml: string;

      if (format === 'markdown') {
        // Heavy step 1: marked.parse() — runs in worker (never blocks main thread)
        rawHtml = await markdownToHtml(text);
      } else {
        // HTML format: WASM already produced HTML.
        // DOMPurify is synchronous — yield first so the overlay paints.
        await nextPaint();
        rawHtml = text;
      }

      if (rev !== renderRevision) return; // user switched tabs — discard

      // Heavy step 2: DOMPurify — synchronous DOM traversal (~100–600 ms for large docs).
      // For markdown, yield again so the overlay is visible before this blocks.
      if (format === 'markdown') await nextPaint();
      if (rev !== renderRevision) return;

      const sanitized = DOMPurify.sanitize(rawHtml);
      if (rev !== renderRevision) return;

      renderCache.set(format, sanitized);
      applyHtml(renderedPane, sanitized);
    } catch (err: unknown) {
      if (rev === renderRevision) {
        store.set('errorMessage', `Render error: ${err}`);
      }
    } finally {
      if (rev === renderRevision) {
        renderBusy = false;
        updateLoadingState();
      }
    }
  }

  // Clear render cache whenever a new PDF is parsed (new outputText).
  store.subscribe('outputText', () => {
    renderCache.clear();
    renderRevision++; // cancel any in-flight render
    updateContent();
  });
  store.subscribe('outputFormat', () => updateContent());
  store.subscribe('darkMode', () => updateTheme());

  // ── Copy / Download ─────────────────────────────────────────────────────────

  actions.querySelector('[data-action="copy"]')!.addEventListener('click', () => {
    navigator.clipboard.writeText(store.get('outputText')).then(
      () => store.set('errorMessage', null),
      () => store.set('errorMessage', 'Failed to copy to clipboard'),
    );
  });

  actions.querySelector('[data-action="download"]')!.addEventListener('click', () => {
    const text   = store.get('outputText');
    const format = store.get('outputFormat');
    const ext    = format === 'json' ? 'json'
                 : format === 'html' ? 'html'
                 : format === 'markdown' ? 'md' : 'txt';
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url  = URL.createObjectURL(blob);
    const a    = document.createElement('a');
    a.href = url;
    a.download = `${store.get('fileName') || 'output'}.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  });

  return container;
}

// ── Theme switch ──────────────────────────────────────────────────────────────

function updateTheme(): void {
  if (!editorView) return;
  const dark   = store.get('darkMode');
  const parent = editorView.dom.parentElement;
  if (!parent) return;

  const doc = editorView.state.doc.toString();
  editorView.destroy();

  const extensions = [
    basicSetup,
    json(),
    EditorView.editable.of(false),
    EditorState.readOnly.of(true),
    EditorView.lineWrapping,
  ];
  if (dark) extensions.push(oneDark);

  editorView = new EditorView({
    state: EditorState.create({ doc, extensions }),
    parent,
  });
}
