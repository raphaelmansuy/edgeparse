#!/usr/bin/env node
/**
 * PP-OCRv6_small host OCR worker (ppu-paddle-ocr + onnxruntime-node).
 * Sync bridge: main thread uses receiveMessageOnPort on a transferred MessagePort.
 */
import { parentPort, workerData } from "node:worker_threads";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
import path from "node:path";

const require = createRequire(
  path.join(workerData?.odlBenchDir || path.resolve(path.dirname(new URL(import.meta.url).pathname), ".."), "package.json"),
);

let service = null;

async function ensureOcr() {
  if (service) return service;
  const entry = pathToFileURL(require.resolve("ppu-paddle-ocr")).href;
  const mod = await import(entry);
  const { PaddleOcrService, V6_SMALL_MODEL } = mod;
  service = new PaddleOcrService({ model: V6_SMALL_MODEL });
  await service.initialize();
  return service;
}

function grayToPngBuffer(gray, width, height) {
  const { PNG } = require("pngjs");
  const png = new PNG({ width, height });
  for (let i = 0; i < width * height; i++) {
    const v = gray[i];
    const o = i << 2;
    png.data[o] = v;
    png.data[o + 1] = v;
    png.data[o + 2] = v;
    png.data[o + 3] = 255;
  }
  return PNG.sync.write(png);
}

function mapWords(result) {
  const words = [];
  const items = result?.results || result?.lines?.flat?.() || [];
  for (const item of items) {
    const text = String(item?.text || "").trim();
    if (!text) continue;
    const box = item.box || {};
    const left = Math.max(0, Math.floor(Number(box.x ?? box.left ?? 0)));
    const top = Math.max(0, Math.floor(Number(box.y ?? box.top ?? 0)));
    const width = Math.max(1, Math.ceil(Number(box.width ?? 1)));
    const height = Math.max(1, Math.ceil(Number(box.height ?? 1)));
    const conf = Number(item.confidence ?? 0.8);
    words.push({
      text,
      left,
      top,
      width,
      height,
      confidence: conf <= 1 ? conf * 100 : conf,
    });
  }
  return words;
}

parentPort.on("message", async (msg) => {
  try {
    if (msg?.type === "init") {
      await ensureOcr();
      parentPort.postMessage({ type: "ready" });
      return;
    }
    if (msg?.type === "ocr") {
      const reply = msg.port;
      try {
        const eng = await ensureOcr();
        const gray =
          msg.gray instanceof Uint8Array ? msg.gray : new Uint8Array(msg.gray);
        const png = grayToPngBuffer(gray, msg.width, msg.height);
        const ab = png.buffer.slice(png.byteOffset, png.byteOffset + png.byteLength);
        const result = await eng.recognize(ab, { flatten: true });
        reply.postMessage({ type: "result", words: mapWords(result) });
      } catch (err) {
        reply.postMessage({
          type: "error",
          message: String(err?.stack || err?.message || err),
        });
      } finally {
        reply.close();
      }
    }
  } catch (err) {
    parentPort.postMessage({
      type: "error",
      message: String(err?.message || err),
    });
  }
});
