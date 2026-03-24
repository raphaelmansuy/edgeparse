/** OverlayCanvas — transparent canvas for bounding box rendering. */

import { el } from '../utils/dom';
import { store } from '../state';
import { drawOverlay, hitTestElements, type DrawnRect } from '../services/overlay-engine';
import type { PageDimensions } from '../types';

let drawnRects: DrawnRect[] = [];
let pageDims: PageDimensions | null = null;

export function createOverlayCanvas(): HTMLCanvasElement {
  const canvas = el('canvas', {
    className: 'overlay-canvas',
  }) as HTMLCanvasElement;

  canvas.addEventListener('mousemove', (e: Event) => {
    const me = e as MouseEvent;
    const rect = canvas.getBoundingClientRect();
    const x = me.clientX - rect.left;
    const y = me.clientY - rect.top;
    const hit = hitTestElements(x, y, drawnRects);
    store.set('hoveredElement', hit);
    canvas.style.cursor = hit ? 'pointer' : 'default';
  });

  canvas.addEventListener('mouseleave', () => {
    store.set('hoveredElement', null);
  });

  // Re-draw when state changes
  const redraw = () => {
    if (!pageDims) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const doc = store.get('parsedDocument');
    if (!doc) {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      drawnRects = [];
      return;
    }
    drawnRects = drawOverlay(
      ctx,
      doc.kids,
      store.get('currentPage'),
      pageDims,
      store.get('activeSemanticFilters'),
      store.get('hoveredElement'),
    );
  };

  store.subscribe('parsedDocument', redraw);
  store.subscribe('currentPage', redraw);
  store.subscribe('hoveredElement', redraw);
  store.subscribe('activeSemanticFilters', redraw); // re-draw immediately when a filter is toggled
  store.subscribe('showOverlay', () => {
    canvas.style.display = store.get('showOverlay') ? '' : 'none';
    if (store.get('showOverlay')) redraw();
  });

  return canvas;
}

/** Called by PdfViewer after rendering a page to sync canvas size and page dims. */
export function syncOverlay(
  canvas: HTMLCanvasElement,
  dims: PageDimensions,
): void {
  pageDims = dims;
  canvas.width = dims.canvasWidth;
  canvas.height = dims.canvasHeight;

  // Trigger redraw
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  const doc = store.get('parsedDocument');
  if (!doc) return;
  drawnRects = drawOverlay(
    ctx,
    doc.kids,
    store.get('currentPage'),
    pageDims,
    store.get('activeSemanticFilters'),
    store.get('hoveredElement'),
  );
}
