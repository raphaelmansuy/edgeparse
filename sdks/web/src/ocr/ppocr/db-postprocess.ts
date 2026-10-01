/** DBNet-style probability map → axis-aligned boxes (connected components). */

import type { Box } from './preprocess.js';

export interface DbOptions {
  threshold?: number;
  minArea?: number;
  padRatio?: number;
}

/**
 * Extract boxes from a detection probability map [H*W] (or [1,1,H,W]).
 * Coordinates are in the resized/padded detection space; caller scales by ratio.
 */
export function boxesFromProbabilityMap(
  probs: ArrayLike<number>,
  width: number,
  height: number,
  opts: DbOptions = {},
): Box[] {
  const threshold = opts.threshold ?? 0.3;
  const minArea = opts.minArea ?? 16;
  const padRatio = opts.padRatio ?? 0.1;
  const n = width * height;
  const visited = new Uint8Array(n);
  const boxes: Box[] = [];

  const idx = (x: number, y: number) => y * width + x;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = idx(x, y);
      if (visited[i] || (probs[i] ?? 0) < threshold) continue;

      // BFS connected component
      let minX = x;
      let maxX = x;
      let minY = y;
      let maxY = y;
      let area = 0;
      const stack = [i];
      visited[i] = 1;
      while (stack.length) {
        const cur = stack.pop()!;
        const cx = cur % width;
        const cy = (cur / width) | 0;
        area += 1;
        if (cx < minX) minX = cx;
        if (cx > maxX) maxX = cx;
        if (cy < minY) minY = cy;
        if (cy > maxY) maxY = cy;
        const neighbors = [
          cur - 1,
          cur + 1,
          cur - width,
          cur + width,
        ];
        for (const nb of neighbors) {
          if (nb < 0 || nb >= n || visited[nb]) continue;
          const nx = nb % width;
          const ny = (nb / width) | 0;
          if (Math.abs(nx - cx) + Math.abs(ny - cy) !== 1) continue;
          if ((probs[nb] ?? 0) < threshold) continue;
          visited[nb] = 1;
          stack.push(nb);
        }
      }

      if (area < minArea) continue;
      let bw = maxX - minX + 1;
      let bh = maxY - minY + 1;
      const padX = Math.round(bh * padRatio);
      const padY = Math.round(bh * padRatio);
      minX = Math.max(0, minX - padX);
      minY = Math.max(0, minY - padY);
      maxX = Math.min(width - 1, maxX + padX);
      maxY = Math.min(height - 1, maxY + padY);
      bw = maxX - minX + 1;
      bh = maxY - minY + 1;
      if (bw < 4 || bh < 4) continue;
      boxes.push({ x: minX, y: minY, width: bw, height: bh });
    }
  }

  // Reading order: top-to-bottom, left-to-right
  boxes.sort((a, b) => {
    if (Math.abs(a.y - b.y) < Math.max(a.height, b.height) * 0.5) {
      return a.x - b.x;
    }
    return a.y - b.y;
  });
  return boxes;
}

/** Map detection-space boxes back to original gray pixel space. */
export function scaleBoxesToOriginal(
  boxes: Box[],
  ratio: number,
  originalWidth: number,
  originalHeight: number,
): Box[] {
  return boxes.map((b) => {
    const x = Math.max(0, Math.round(b.x / ratio));
    const y = Math.max(0, Math.round(b.y / ratio));
    const width = Math.min(originalWidth - x, Math.round(b.width / ratio));
    const height = Math.min(originalHeight - y, Math.round(b.height / ratio));
    return { x, y, width: Math.max(1, width), height: Math.max(1, height) };
  });
}
