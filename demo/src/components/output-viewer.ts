/**
 * OutputViewer — right pane.
 *
 * UX design for format switching (non-blocking feedback):
 *  • JSON / Text   → CodeMirror instant update (always fast, no loading state)
 *  • Markdown      → marked.parse() in render-worker (off-thread); DOMPurify
 *                    after double-rAF so the shimmer bar is visible first
 *  • HTML          → DOMPurify after double-rAF so the shimmer bar paints first
 *
 * Loading feedback strategy:
 *  • PDF parsing        → full-screen overlay (user can't interact anyway)
 *  • Format rendering   → thin animated shimmer bar (top of pane) +
 *                         rendered pane dims to 50% opacity (keeps old content
 *                         visible so the screen never goes blank) +
 *                         active tab shows a pulse dot (via renderStatus key)
 *
 * renderCache (Map<format → sanitized HTML>) is cleared on new PDF load.
 * renderRevision guards against race conditions on rapid tab switching.
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

/**
 * Yield two animation frames so pending paints flush before blocking work.
 * Double-rAF ensures the browser has actually committed a paint cycle:
 * first rAF fires before the paint, second fires after it.
 */
function nextPaint(): Promise<void> {
  return new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r())));
}

/**
 * Inject pre-sanitized HTML via an inert <template> parse.
 * Caller must ensure `sanitizedHtml` has already been through DOMPurify —
 * this function does NOT double-sanitize.
 */
function setSanitizedHtml(target: HTMLElement, sanitizedHtml: string): void {
  const tmpl = document.createElement('template');
  tmpl.innerHTML = sanitizedHtml;
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

// True while an async render (worker + DOMPurify) is in flight.
let renderBusy = false;

// ── Component ─────────────────────────────────────────────────────────────────

export function createOutputViewer(): HTMLElement {
  const container = el('div', { className: 'output-viewer' });

  const codePane = el('div', { className: 'output-viewer__code' });
  const renderedPane = el('div', {
    className: 'output-viewer__rendered',
    style: 'display: none;',
  });

  // Thin shimmer bar shown during format rendering (NOT during PDF parsing).
  // Replaces the full overlay for format switches so old content stays visible.
  const renderProgress = el('div', {
    className: 'output-viewer__render-progress',
    ariaHidden: 'true',
  });

  const actions = el('div', {
    className: 'output-viewer__actions',
    innerHTML: `
      <button class="output-viewer__btn" data-action="copy"     aria-label="Copy output">Copy</button>
      <button class="output-viewer__btn" data-action="download" aria-label="Download output">Download</button>
    `,
  });

  // Full-screen overlay — only for active PDF parsing (not format switching).
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

  // Layout: actions bar → render progress bar → content panes → parse overlay
  container.append(actions, renderProgress, codePane, renderedPane, loadingOverlay);

  // CodeMirror — read-only, line-wrapping, no theme until user picks dark mode
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

  // ── Loading state management ────────────────────────────────────────────────

  function updateLoadingState(): void {
    const ws  = store.get('wasmStatus');
    const ps  = store.get('parseStatus');
    const has = !!store.get('pdfBytes');
    const parseBusy = ps === 'parsing' || (ws === 'loading' && has);

    // Full overlay: only while parsing a PDF (user cannot interact anyway).
    loadingOverlay.style.display = parseBusy ? '' : 'none';
    if (parseBusy) {
      (loadingOverlay.querySelector('.loading-text') as HTMLElement).textContent =
        ws === 'loading' ? 'Loading parser…' : 'Parsing PDF…';
    }

    // Thin shimmer bar: only while rendering a format (never during parse).
    // Shown alongside dimmed content — user can still see what was there before.
    const showProgress = !parseBusy && renderBusy;
    renderProgress.classList.toggle('output-viewer__render-progress--active', showProgress);

    // Propagate render status to store so format-tabs can show a per-tab indicator.
    store.set('renderStatus', renderBusy ? 'rendering' : 'idle');
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

    // Show/hide the correct pane.
    const codePaneEl = editorView?.dom.parentElement;
    if (codePaneEl) codePaneEl.style.display = isCode ? '' : 'none';
    renderedPane.style.display = isCode ? 'none' : '';

    if (isCode) {
      // Cancel any in-flight render; clean up dimming state.
      renderBusy = false;
      renderRevision++;
      renderedPane.classList.remove('output-viewer__rendered--loading');
      updateLoadingState();
      editorView?.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: text },
      });
      return;
    }

    // ── Rendered view (markdown | html) ──────────────────────────────────────

    const cached = renderCache.get(format);
    if (cached !== undefined) {
      // Cache hit: cancel in-flight render, serve immediately, no overlay.
      renderBusy = false;
      renderRevision++;
      renderedPane.classList.remove('output-viewer__rendered--loading');
      updateLoadingState();
      setSanitizedHtml(renderedPane, cached);
      return;
    }

    // Cache miss: start async render with non-blocking feedback.
    // Old content stays visible at 50% opacity while the new one renders.
    const rev = ++renderRevision;
    renderBusy = true;
    renderedPane.classList.add('output-viewer__rendered--loading');
    updateLoadingState();

    try {
      let rawHtml: string;

      if (format === 'markdown') {
        // Worker converts markdown→HTML off-thread (main thread stays free).
        rawHtml = await markdownToHtml(text);
      } else {
        // HTML format: WASM already produced HTML; just need to sanitize.
        // Yield so the shimmer bar + dim are guaranteed to paint first.
        await nextPaint();
        rawHtml = text;
      }

      if (rev !== renderRevision) return; // user switched tabs — discard

      // DOMPurify is synchronous (~100–600 ms on large docs). Yield one more
      // paint so the shimmer bar remains visible during this blocking call.
      if (format === 'markdown') await nextPaint();
      if (rev !== renderRevision) return;

      const sanitized = DOMPurify.sanitize(rawHtml);
      if (rev !== renderRevision) return;

      renderCache.set(format, sanitized);

      // Swap content then fade in via CSS transition (class removal → opacity 1).
      setSanitizedHtml(renderedPane, sanitized);
      renderedPane.classList.remove('output-viewer__rendered--loading');
    } catch (err: unknown) {
      if (rev === renderRevision) {
        renderedPane.classList.remove('output-viewer__rendered--loading');
        store.set('errorMessage', `Render error: ${err}`);
      }
    } finally {
      if (rev === renderRevision) {
        renderBusy = false;
        updateLoadingState();
      }
    }
  }

  // Clear render cache ONLY when a new PDF is loaded (formatCache changes from
  // the WASM parse worker). Format switches do NOT clear it — so switching back
  // to a previously-rendered format is always an instant cache hit.
  store.subscribe('formatCache', () => {
    renderCache.clear();
    renderRevision++; // cancel any in-flight render from the previous PDF
  });

  // Re-render whenever the output text changes (handles both format switches
  // and new PDF loads — by the time this fires, outputFormat is already the
  // new value so both reads are consistent).
  store.subscribe('outputText', () => updateContent());
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
