/** OutputViewer — right pane: CodeMirror editor for JSON/Text, rendered HTML/Markdown. */

import { el } from '../utils/dom';
import { store } from '../state';
import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { json } from '@codemirror/lang-json';
import { oneDark } from '@codemirror/theme-one-dark';
import { marked } from 'marked';
import DOMPurify from 'dompurify';

let editorView: EditorView | null = null;

export function createOutputViewer(): HTMLElement {
  const container = el('div', { className: 'output-viewer' });

  // Code pane (CodeMirror)
  const codePane = el('div', { className: 'output-viewer__code' });
  // Rendered pane (HTML / Markdown)
  const renderedPane = el('div', {
    className: 'output-viewer__rendered',
    style: 'display: none;',
  });

  // Copy & Download buttons
  const actions = el('div', {
    className: 'output-viewer__actions',
    innerHTML: `
      <button class="output-viewer__btn" data-action="copy" aria-label="Copy output">Copy</button>
      <button class="output-viewer__btn" data-action="download" aria-label="Download output">Download</button>
    `,
  });

  // Loading overlay — shown while WASM initialises or PDF is being parsed
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

  // CodeMirror setup
  const state = EditorState.create({
    doc: '',
    extensions: [
      basicSetup,
      json(),
      EditorView.editable.of(false),
      EditorState.readOnly.of(true),
      EditorView.lineWrapping,
    ],
  });

  editorView = new EditorView({ state, parent: codePane });

  // State subscriptions
  store.subscribe('outputText', () => updateContent(renderedPane));
  store.subscribe('outputFormat', () => updateContent(renderedPane));
  store.subscribe('darkMode', () => updateTheme());

  function updateLoadingState() {
    const wasmStatus = store.get('wasmStatus');
    const parseStatus = store.get('parseStatus');
    const hasPdf = !!store.get('pdfBytes');
    // Show overlay only when actively processing — not during background WASM pre-warm
    const busy = parseStatus === 'parsing' || (wasmStatus === 'loading' && hasPdf);
    loadingOverlay.style.display = busy ? '' : 'none';
    const msg = loadingOverlay.querySelector('.loading-text') as HTMLElement | null;
    if (msg) {
      msg.textContent = wasmStatus === 'loading' ? 'Loading parser…' : 'Parsing PDF…';
    }
  }
  store.subscribe('wasmStatus', updateLoadingState);
  store.subscribe('parseStatus', updateLoadingState);
  store.subscribe('pdfBytes', updateLoadingState);
  updateLoadingState();

  // Copy button
  actions.querySelector('[data-action="copy"]')!.addEventListener('click', () => {
    const text = store.get('outputText');
    navigator.clipboard.writeText(text).then(
      () => store.set('errorMessage', null), // could show toast "Copied!"
      () => store.set('errorMessage', 'Failed to copy to clipboard'),
    );
  });

  // Download button
  actions.querySelector('[data-action="download"]')!.addEventListener('click', () => {
    const text = store.get('outputText');
    const format = store.get('outputFormat');
    const ext = format === 'json' ? 'json' : format === 'html' ? 'html' : format === 'markdown' ? 'md' : 'txt';
    const blob = new Blob([text], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${store.get('fileName') || 'output'}.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  });

  return container;
}

function updateContent(renderedPane: HTMLElement): void {
  const text = store.get('outputText');
  const format = store.get('outputFormat');

  const codeFormats = ['json', 'text'];
  const showCode = codeFormats.includes(format);

  if (editorView) {
    const parent = editorView.dom.parentElement;
    if (parent) parent.style.display = showCode ? '' : 'none';
  }
  renderedPane.style.display = showCode ? 'none' : '';

  if (showCode && editorView) {
    editorView.dispatch({
      changes: {
        from: 0,
        to: editorView.state.doc.length,
        insert: text,
      },
    });
  } else if (format === 'markdown') {
    const html = marked.parse(text);
    if (typeof html === 'string') {
      renderedPane.innerHTML = DOMPurify.sanitize(html);
    }
  } else if (format === 'html') {
    renderedPane.innerHTML = DOMPurify.sanitize(text);
  }
}

function updateTheme(): void {
  if (!editorView) return;
  const dark = store.get('darkMode');
  // Recreate with theme
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

  const state = EditorState.create({ doc, extensions });
  editorView = new EditorView({ state, parent });
}
