/** Toolbar — file upload button, format selector, overlay toggle, dark mode. */

import { el } from '../utils/dom';
import { store } from '../state';
import { createFormatTabs } from './format-tabs';

export function createToolbar(): HTMLElement {
  const header = el('header', { className: 'toolbar' });

  // Logo / title
  const brand = el('div', {
    className: 'toolbar__brand',
    innerHTML: `<span class="toolbar__logo">EP</span><span class="toolbar__title">EdgeParse Demo</span>`,
  });

  // File upload — use <label> so the browser natively opens the file chooser
  // on click. Avoids the programmatic fileInput.click() pattern which is
  // blocked by Safari and some Chromium security policies when the input is
  // display:none.
  const fileInput = el('input', {
    id: 'pdf-upload-input',
    type: 'file',
    accept: '.pdf,application/pdf',
    className: 'toolbar__file-input',
  }) as HTMLInputElement;

  const uploadBtn = el('label', {
    className: 'toolbar__btn toolbar__btn--upload',
    ariaLabel: 'Upload a PDF file',
    role: 'button',
    tabindex: '0',
  }, 'Upload PDF', fileInput) as HTMLLabelElement;

  // Keyboard activation (Enter / Space) for the label acting as a button.
  uploadBtn.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      fileInput.click();
    }
  });

  fileInput.addEventListener('change', () => {
    const file = fileInput.files?.[0];
    if (!file) return;
    handleFileUpload(file);
    fileInput.value = '';
  });

  // Overlay toggle
  const overlayBtn = el('button', {
    className: 'toolbar__btn toolbar__btn--overlay toolbar__btn--active',
    textContent: 'Overlay',
    ariaLabel: 'Toggle bounding box overlay',
    ariaPressed: 'true',
  }) as HTMLButtonElement;

  overlayBtn.addEventListener('click', () => {
    const next = !store.get('showOverlay');
    store.set('showOverlay', next);
    overlayBtn.classList.toggle('toolbar__btn--active', next);
    overlayBtn.setAttribute('aria-pressed', String(next));
  });

  // Dark mode toggle
  const darkBtn = el('button', {
    className: 'toolbar__btn toolbar__btn--dark',
    textContent: '🌙',
    ariaLabel: 'Toggle dark mode',
  }) as HTMLButtonElement;

  darkBtn.addEventListener('click', () => {
    const next = !store.get('darkMode');
    store.set('darkMode', next);
    document.documentElement.classList.toggle('dark', next);
    darkBtn.textContent = next ? '☀️' : '🌙';
  });

  const formatTabs = createFormatTabs();

  const div1 = el('span', { className: 'toolbar__divider', ariaHidden: 'true' });
  const div2 = el('span', { className: 'toolbar__divider', ariaHidden: 'true' });

  const actions = el('div', { className: 'toolbar__actions' });
  actions.append(uploadBtn, div1, formatTabs, div2, overlayBtn, darkBtn);

  header.append(brand, actions);
  return header;
}

async function handleFileUpload(file: File): Promise<void> {
  try {
    store.set('errorMessage', null);
    const buffer = await file.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    store.set('fileName', file.name);
    store.set('pdfBytes', bytes);
    // Do NOT set 'currentPage' here — onPdfLoaded (pdf-viewer) handles that
    // once the new PDF is fully loaded. Setting it here races with the async
    // loadPdf() call which nulls out pdfDoc before the new one is ready,
    // causing a spurious "No PDF loaded" error.
  } catch (err: unknown) {
    store.set('errorMessage', `Failed to read file: ${err}`);
  }
}

/** Enable drag-and-drop on the document body. */
export function enableDragDrop(): void {
  const body = document.body;

  body.addEventListener('dragover', (e) => {
    e.preventDefault();
    body.classList.add('drag-over');
  });

  body.addEventListener('dragleave', () => {
    body.classList.remove('drag-over');
  });

  body.addEventListener('drop', (e) => {
    e.preventDefault();
    body.classList.remove('drag-over');
    const file = e.dataTransfer?.files[0];
    if (file && file.type === 'application/pdf') {
      handleFileUpload(file);
    }
  });
}
