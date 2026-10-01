#!/usr/bin/env node
/**
 * Score EdgeParse WASM on odl-bench PDFs → prediction/edgeparse_wasm/markdown/
 *
 *   node odl-bench/scripts/run_wasm_bench.mjs [--max N] [--no-ocr] [--ocrs-only]
 *
 * OCR preference:
 *   1. Host PP-OCRv6_small via @paddleocr/paddleocr-js (Worker + MessagePort sync)
 *   2. Fallback: in-wasm ocrs when host returns empty / unavailable
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync } from "node:fs";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import os from "node:os";
import {
  Worker,
  MessageChannel,
  receiveMessageOnPort,
} from "node:worker_threads";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const pkgDir = path.join(root, "crates/edgeparse-wasm/pkg");
const pdfDir = path.join(root, "odl-bench/pdfs");
const outDir = path.join(root, "odl-bench/prediction/edgeparse_wasm/markdown");
const odlNodeModules = path.join(root, "odl-bench/node_modules");


const maxArg = process.argv.indexOf("--max");
const maxDocs = maxArg >= 0 ? Number(process.argv[maxArg + 1]) : 0;
const noOcr = process.argv.includes("--no-ocr");
const ocrsOnly = process.argv.includes("--ocrs-only");

const entry = path.join(pkgDir, "edgeparse_wasm.js");
if (!existsSync(entry)) {
  console.error(
    "Missing wasm pkg. Run: wasm-pack build crates/edgeparse-wasm --target nodejs --out-dir pkg --release --features ocr-ocrs",
  );
  process.exit(1);
}

mkdirSync(outDir, { recursive: true });
const mod = await import(pathToFileURL(entry).href);
if (typeof mod.default === "function") await mod.default();
else if (mod.init) await mod.init();

let ocrReady = false;
let hostOcr = false;
let ocrsLoaded = false;
/** @type {{ recognizeSync: Function, dispose: Function } | null} */
let ppocrBridge = null;

function sleepSyncMs(ms) {
  const sab = new SharedArrayBuffer(4);
  const lock = new Int32Array(sab);
  Atomics.wait(lock, 0, 0, ms);
}

async function tryInitPpocrHost() {
  if (noOcr || ocrsOnly || typeof mod.register_ocr_backend !== "function") {
    return false;
  }
  const ppuEntry = path.join(odlNodeModules, "ppu-paddle-ocr/package.json");
  if (!existsSync(ppuEntry)) {
    console.warn(
      "Install host OCR: cd odl-bench && npm i ppu-paddle-ocr onnxruntime-node pngjs",
    );
    return false;
  }

  const worker = new Worker(path.join(__dirname, "ppocr_worker.mjs"), {
    workerData: { odlBenchDir: path.join(root, "odl-bench") },
  });

  await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("PP-OCR worker init timeout")), 180_000);
    worker.once("message", (msg) => {
      if (msg?.type === "ready") {
        clearTimeout(t);
        resolve();
      } else if (msg?.type === "error") {
        clearTimeout(t);
        reject(new Error(msg.message || "init failed"));
      }
    });
    worker.once("error", (err) => {
      clearTimeout(t);
      reject(err);
    });
    worker.postMessage({ type: "init" });
  });

  console.log("PP-OCRv6_small host OCR ready (ppu-paddle-ocr)");

  ppocrBridge = {
    recognizeSync(gray, width, height) {
      const { port1, port2 } = new MessageChannel();
      const copy = gray instanceof Uint8Array ? gray.slice() : new Uint8Array(gray);
      worker.postMessage(
        { type: "ocr", gray: copy, width, height, port: port2 },
        [copy.buffer, port2],
      );
      const deadline = Date.now() + 120_000;
      while (Date.now() < deadline) {
        const msg = receiveMessageOnPort(port1);
        if (msg) {
          port1.close();
          if (msg.message?.type === "error") {
            console.warn("PP-OCR error:", msg.message.message);
            return [];
          }
          return msg.message?.words || [];
        }
        sleepSyncMs(5);
      }
      port1.close();
      console.warn("PP-OCR timed out");
      return [];
    },
    dispose() {
      worker.terminate();
    },
  };

  mod.register_ocr_backend((gray, width, height) =>
    ppocrBridge.recognizeSync(gray, width, height),
  );
  hostOcr = true;
  ocrReady = true;
  return true;
}

function tryInitOcrs() {
  if (noOcr || typeof mod.init_ocrs_models !== "function") {
    return false;
  }
  const modelDir =
    process.env.EDGEPARSE_OCRS_MODEL_DIR ||
    path.join(os.homedir(), ".cache/edgeparse/ocrs");
  const det = path.join(modelDir, "text-detection.rten");
  const rec = path.join(modelDir, "text-recognition.rten");
  if (!(existsSync(det) && existsSync(rec))) {
    console.warn(`ocrs models missing under ${modelDir}`);
    return false;
  }
  mod.init_ocrs_models(readFileSync(det), readFileSync(rec));
  ocrsLoaded = true;
  ocrReady = true;
  console.log(`ocrs models loaded from ${modelDir}`);
  return true;
}

if (!noOcr) {
  try {
    const okHost = await tryInitPpocrHost();
    if (!okHost) {
      tryInitOcrs();
    } else {
      tryInitOcrs();
    }
  } catch (err) {
    console.warn("PP-OCR host init failed:", err?.message || err);
    tryInitOcrs();
  }
} else {
  console.warn("--no-ocr: lattice uses PDF text only");
}

let pdfs = readdirSync(pdfDir).filter((f) => f.endsWith(".pdf")).sort();
if (maxDocs > 0) pdfs = pdfs.slice(0, maxDocs);

let ok = 0;
let fail = 0;
const t0 = performance.now();
const failures = [];

for (let i = 0; i < pdfs.length; i++) {
  const name = pdfs[i];
  const stem = name.replace(/\.pdf$/i, "");
  const bytes = readFileSync(path.join(pdfDir, name));
  try {
    const md = mod.convert_to_string(bytes, "markdown", null, null, "cluster");
    writeFileSync(path.join(outDir, `${stem}.md`), md ?? "", "utf8");
    ok++;
  } catch (err) {
    fail++;
    failures.push({ stem, err: String(err?.message || err) });
    writeFileSync(path.join(outDir, `${stem}.md`), "", "utf8");
  }
  if ((i + 1) % 20 === 0 || i === pdfs.length - 1) {
    const elapsed = ((performance.now() - t0) / 1000).toFixed(1);
    console.log(`[${i + 1}/${pdfs.length}] ok=${ok} fail=${fail} ${elapsed}s`);
  }
}

ppocrBridge?.dispose();

const summary = {
  engine_name: "edgeparse_wasm",
  engine_version: typeof mod.version === "function" ? mod.version() : "wasm",
  document_count: pdfs.length,
  ok,
  fail,
  failures: failures.slice(0, 20),
  total_elapsed_sec: (performance.now() - t0) / 1000,
  ocr_ocrs: ocrsLoaded,
  ocr_host_ppocrv6: hostOcr,
  note: hostOcr
    ? "WASM convert_bytes + inmem lattice + host PP-OCRv6_small; heading refine"
    : ocrsLoaded
      ? "WASM convert_bytes + inmem lattice + ocrs; heading refine"
      : "WASM convert_bytes + inmem lattice (PDF text fill); heading refine",
};
writeFileSync(
  path.join(root, "odl-bench/prediction/edgeparse_wasm/summary.json"),
  JSON.stringify(summary, null, 2),
);
console.log(JSON.stringify(summary, null, 2));
