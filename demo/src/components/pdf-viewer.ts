/** PdfViewer — left pane: PDF.js canvas + overlay. */

import { el } from '../utils/dom';
import { store } from '../state';
import { loadPdf, renderPage, getPageCount } from '../services/pdf-renderer';
import { createOverlayCanvas, syncOverlay } from './overlay-canvas';
import { createPageNav } from './page-nav';
import { createLegend } from './legend';

const SCALE = 1.5;

export function createPdfViewer(): HTMLElement {
  const container = el('div', { className: 'pdf-viewer' });
  const canvasWrapper = el('div', { className: 'pdf-viewer__canvas-wrapper' });
  const pdfCanvas = el('canvas', { className: 'pdf-viewer__canvas' }) as HTMLCanvasElement;
  const overlayCanvas = createOverlayCanvas();
  const pageNav = createPageNav();
  const legend = createLegend();
  const dropHint = el('div', { className: 'pdf-viewer__drop-zone' });
  dropHint.innerHTML = `
    <label class="pdf-viewer__drop-zone-inner" for="pdf-upload-input"
           aria-label="Click or drop a PDF file here to upload">
      <svg class="drop-zone__icon" aria-hidden="true" viewBox="0 0 24 24"
           fill="none" stroke="currentColor" stroke-width="1.5"
           stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 16.5V9.75m0 0-3 3m3-3 3 3"/>
        <path d="M6.75 19.5a4.5 4.5 0 0 1-1.41-8.775
                 5.25 5.25 0 0 1 10.338-2.227
                 5.25 5.25 0 0 1 1.14 10.003H6.75z"/>
      </svg>
      <strong class="drop-zone__title">Drop your PDF here</strong>
      <span class="drop-zone__sub">or <span class="drop-zone__link">browse files</span></span>
      <span class="drop-zone__hint">PDF files only</span>
    </label>
  `;

  canvasWrapper.append(pdfCanvas, overlayCanvas);
  container.append(dropHint, canvasWrapper, legend, pageNav);

  // Hide canvas wrapper and page nav until a PDF is loaded
  canvasWrapper.style.display = 'none';
  pageNav.style.display = 'none';

  let rendering = false;
  let pendingRender = false;

  async function onPdfLoaded() {
    const bytes = store.get('pdfBytes');
    if (!bytes) {
      dropHint.style.display = '';
      canvasWrapper.style.display = 'none';
      pageNav.style.display = 'none';
      return;
    }

    dropHint.style.display = 'none';
    canvasWrapper.style.display = '';
    pageNav.style.display = '';

    try {
      await loadPdf(bytes);
      store.set('pageCount', getPageCount());
      // Setting currentPage triggers the subscription which renders.
      // Don't call renderCurrentPage here to avoid double-render.
      store.set('currentPage', 1);
    } catch (err: unknown) {
      store.set('errorMessage', `PDF render error: ${err}`);
    }
  }

  async function renderCurrentPage(
    pdf: HTMLCanvasElement,
    overlay: HTMLCanvasElement,
  ) {
    const page = store.get('currentPage');
    if (!page || store.get('pageCount') === 0) return;

    if (rendering) {
      pendingRender = true;
      return;
    }
    rendering = true;

    try {
      const { pdfWidth, pdfHeight } = await renderPage(page, pdf, SCALE);
      if (pdfWidth === 0) return; // Cancelled

      syncOverlay(overlay, {
        pdfWidth,
        pdfHeight,
        canvasWidth: pdf.width,
        canvasHeight: pdf.height,
        scale: SCALE,
      });
    } catch (err: unknown) {
      store.set('errorMessage', `Page render error: ${err}`);
    } finally {
      rendering = false;
      if (pendingRender) {
        pendingRender = false;
        renderCurrentPage(pdf, overlay);
      }
    }
  }

  store.subscribe('pdfBytes', onPdfLoaded);
  store.subscribe('currentPage', () =>
    renderCurrentPage(pdfCanvas, overlayCanvas),
  );

  return container;
}
