/** PDF.js rendering orchestrator. */

import * as pdfjsLib from 'pdfjs-dist';
import type { PDFDocumentProxy, PDFPageProxy, RenderTask } from 'pdfjs-dist';

pdfjsLib.GlobalWorkerOptions.workerSrc = './worker/pdf.worker.min.mjs';

let pdfDoc: PDFDocumentProxy | null = null;
let currentRenderTask: RenderTask | null = null;

export async function loadPdf(data: Uint8Array): Promise<PDFDocumentProxy> {
  if (pdfDoc) {
    pdfDoc.destroy();
    pdfDoc = null;
  }
  // Copy to prevent PDF.js from detaching the original ArrayBuffer
  const copy = new Uint8Array(data);
  pdfDoc = await pdfjsLib.getDocument({ data: copy }).promise;
  return pdfDoc;
}

export function getPageCount(): number {
  return pdfDoc?.numPages ?? 0;
}

export async function renderPage(
  pageNum: number,
  canvas: HTMLCanvasElement,
  scale = 1.5,
): Promise<{ pdfWidth: number; pdfHeight: number }> {
  if (!pdfDoc) throw new Error('No PDF loaded');
  if (pageNum < 1 || pageNum > pdfDoc.numPages) {
    throw new Error(`Invalid page number: ${pageNum}`);
  }

  // Cancel any in-progress render
  if (currentRenderTask) {
    currentRenderTask.cancel();
    currentRenderTask = null;
  }

  const page: PDFPageProxy = await pdfDoc.getPage(pageNum);
  const viewport = page.getViewport({ scale });

  canvas.width = viewport.width;
  canvas.height = viewport.height;

  currentRenderTask = page.render({ canvas, viewport });

  try {
    await currentRenderTask.promise;
  } catch (err: unknown) {
    // Cancelled renders are not errors
    if (err instanceof Error && err.message.includes('Rendering cancelled')) return { pdfWidth: 0, pdfHeight: 0 };
    throw err;
  } finally {
    currentRenderTask = null;
  }

  return {
    pdfWidth: page.getViewport({ scale: 1 }).width,
    pdfHeight: page.getViewport({ scale: 1 }).height,
  };
}

export function destroy(): void {
  if (currentRenderTask) {
    currentRenderTask.cancel();
    currentRenderTask = null;
  }
  if (pdfDoc) {
    pdfDoc.destroy();
    pdfDoc = null;
  }
}
