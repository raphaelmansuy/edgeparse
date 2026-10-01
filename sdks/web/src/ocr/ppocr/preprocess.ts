/** Detection / recognition image tensors for PP-OCR (matches ppu-paddle-ocr). */

export interface DetResize {
  width: number;
  height: number;
  ratio: number;
  tensor: Float32Array; // NCHW [1,3,H,W]
}

export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Resize so longest side ≤ maxSide (multiple of 32), return ImageNet-normalized tensor. */
export function prepareDetection(
  gray: Uint8Array,
  srcW: number,
  srcH: number,
  maxSide = 960,
): DetResize {
  const longest = Math.max(srcW, srcH);
  let ratio = 1;
  let width = srcW;
  let height = srcH;
  if (longest > maxSide) {
    ratio = maxSide / longest;
    width = Math.max(32, Math.round(srcW * ratio));
    height = Math.max(32, Math.round(srcH * ratio));
  }
  // Pad to multiple of 32
  const padW = Math.ceil(width / 32) * 32;
  const padH = Math.ceil(height / 32) * 32;

  const rgba = grayToRgbaResized(gray, srcW, srcH, width, height, padW, padH);
  const mean = [0.485, 0.456, 0.406];
  const std = [0.229, 0.224, 0.225];
  const channelSize = padW * padH;
  const tensor = new Float32Array(3 * channelSize);
  const scale = [1 / (255 * std[0]!), 1 / (255 * std[1]!), 1 / (255 * std[2]!)];
  const shift = [mean[0]! / std[0]!, mean[1]! / std[1]!, mean[2]! / std[2]!];
  for (let i = 0, p = 0; i < channelSize; i++, p += 4) {
    tensor[i] = rgba[p]! * scale[0]! - shift[0]!;
    tensor[channelSize + i] = rgba[p + 1]! * scale[1]! - shift[1]!;
    tensor[2 * channelSize + i] = rgba[p + 2]! * scale[2]! - shift[2]!;
  }
  return { width: padW, height: padH, ratio: width / srcW, tensor };
}

/** Recognition crop → CHW float [-1,1], height = 48. */
export function prepareRecognition(
  gray: Uint8Array,
  srcW: number,
  srcH: number,
  box: Box,
  targetHeight = 48,
): { tensor: Float32Array; width: number; height: number } {
  const x0 = Math.max(0, Math.floor(box.x));
  const y0 = Math.max(0, Math.floor(box.y));
  const x1 = Math.min(srcW, Math.ceil(box.x + box.width));
  const y1 = Math.min(srcH, Math.ceil(box.y + box.height));
  const cropW = Math.max(1, x1 - x0);
  const cropH = Math.max(1, y1 - y0);
  const aspect = cropW / cropH;
  const resizedW = Math.max(8, Math.round(targetHeight * aspect));

  const rgba = new Uint8ClampedArray(resizedW * targetHeight * 4);
  for (let y = 0; y < targetHeight; y++) {
    const sy = y0 + Math.min(cropH - 1, Math.floor((y / targetHeight) * cropH));
    for (let x = 0; x < resizedW; x++) {
      const sx = x0 + Math.min(cropW - 1, Math.floor((x / resizedW) * cropW));
      const v = gray[sy * srcW + sx] ?? 255;
      const o = (y * resizedW + x) * 4;
      rgba[o] = v;
      rgba[o + 1] = v;
      rgba[o + 2] = v;
      rgba[o + 3] = 255;
    }
  }

  const channelSize = resizedW * targetHeight;
  const tensor = new Float32Array(3 * channelSize);
  const inv = 1 / 127.5;
  for (let i = 0, p = 0; i < channelSize; i++, p += 4) {
    const n = rgba[p]! * inv - 1;
    tensor[i] = n;
    tensor[channelSize + i] = n;
    tensor[2 * channelSize + i] = n;
  }
  return { tensor, width: resizedW, height: targetHeight };
}

function grayToRgbaResized(
  gray: Uint8Array,
  srcW: number,
  srcH: number,
  dstW: number,
  dstH: number,
  padW: number,
  padH: number,
): Uint8ClampedArray {
  const out = new Uint8ClampedArray(padW * padH * 4);
  // fill white
  out.fill(255);
  for (let y = 0; y < dstH; y++) {
    const sy = Math.min(srcH - 1, Math.floor((y / dstH) * srcH));
    for (let x = 0; x < dstW; x++) {
      const sx = Math.min(srcW - 1, Math.floor((x / dstW) * srcW));
      const v = gray[sy * srcW + sx] ?? 255;
      const o = (y * padW + x) * 4;
      out[o] = v;
      out[o + 1] = v;
      out[o + 2] = v;
      out[o + 3] = 255;
    }
  }
  return out;
}
