/** Bounding-box overlay engine — renders content element boxes on a transparent canvas. */

import type { ContentElement, PageDimensions, SemanticType } from '../types';
import { pdfToScreen, type ScreenRect } from '../utils/coords';
import {
  getSemanticType,
  flattenElement,
  SEMANTIC_COLORS,
  SEMANTIC_BORDER_COLORS,
} from '../utils/colors';

export interface DrawnRect {
  rect: ScreenRect;
  element: ContentElement;
}

export function drawOverlay(
  ctx: CanvasRenderingContext2D,
  elements: ContentElement[],
  pageNum: number,
  pageDims: PageDimensions,
  activeFilters: Set<SemanticType>,
  hoveredElement: ContentElement | null,
): DrawnRect[] {
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  const drawn: DrawnRect[] = [];

  for (const el of elements) {
    const flat = flattenElement(el);
    if (!flat) continue;

    const elPage = flat.bbox.page_number ?? 0;
    if (elPage !== pageNum) continue;

    const semType = getSemanticType(flat.type);
    if (!activeFilters.has(semType)) continue;

    const rect = pdfToScreen(flat.bbox, pageDims);
    if (rect.w <= 0 || rect.h <= 0) continue;

    const isHovered = hoveredElement === el;

    ctx.fillStyle = isHovered
      ? SEMANTIC_BORDER_COLORS[semType]
      : SEMANTIC_COLORS[semType];
    ctx.fillRect(rect.x, rect.y, rect.w, rect.h);

    ctx.strokeStyle = SEMANTIC_BORDER_COLORS[semType];
    ctx.lineWidth = isHovered ? 2 : 1;
    ctx.strokeRect(rect.x, rect.y, rect.w, rect.h);

    drawn.push({ rect, element: el });
  }

  return drawn;
}

export function hitTestElements(
  x: number,
  y: number,
  rects: DrawnRect[],
): ContentElement | null {
  for (let i = rects.length - 1; i >= 0; i--) {
    const { rect } = rects[i];
    if (x >= rect.x && x <= rect.x + rect.w && y >= rect.y && y <= rect.y + rect.h) {
      return rects[i].element;
    }
  }
  return null;
}
