/** PDF coordinate ↔ screen pixel transforms. */

import type { BBox, PageDimensions } from '../types';

export interface ScreenRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * Convert PDF-space bbox (origin bottom-left) to screen-space rect (origin top-left).
 * BBox uses left_x, bottom_y, right_x, top_y in PDF coordinates.
 */
export function pdfToScreen(
  bbox: BBox,
  page: PageDimensions,
): ScreenRect {
  const scaleX = page.canvasWidth / page.pdfWidth;
  const scaleY = page.canvasHeight / page.pdfHeight;

  const x = bbox.left_x * scaleX;
  const y = (page.pdfHeight - bbox.top_y) * scaleY;
  const w = (bbox.right_x - bbox.left_x) * scaleX;
  const h = (bbox.top_y - bbox.bottom_y) * scaleY;

  return { x, y, w, h };
}

/**
 * Test whether a screen-space point falls inside a bbox (already in screen coords).
 */
export function hitTest(
  px: number,
  py: number,
  rect: { x: number; y: number; w: number; h: number },
): boolean {
  return px >= rect.x && px <= rect.x + rect.w && py >= rect.y && py <= rect.y + rect.h;
}
