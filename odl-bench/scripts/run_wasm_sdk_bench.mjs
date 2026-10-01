#!/usr/bin/env node
/**
 * Score EdgeParse WASM via @edgeparse/web Node adapter (product path).
 *
 *   node odl-bench/scripts/run_wasm_sdk_bench.mjs [--max N] [--no-ocr]
 *
 * Uses ParseSession two-phase + optional PP-OCR host backend so the board
 * measures what the Web SDK delivers.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync } from "node:fs";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";
import {
  Worker,
  MessageChannel,
  receiveMessageOnPort,
} from "node:worker_threads";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const pkgNodeDir = path.join(root, "crates/edgeparse-wasm/pkg-node");
const pkgWebDir = path.join(root, "crates/edgeparse-wasm/pkg");
const pdfDir = path.join(root, "odl-bench/pdfs");
const outDir = path.join(root, "odl-bench/prediction/edgeparse_wasm/markdown");
const odlNodeModules = path.join(root, "odl-bench/node_modules");
const sdkNode = path.join(root, "sdks/web/src/node/index.ts");

const maxArg = process.argv.indexOf("--max");
const maxDocs = maxArg >= 0 ? Number(process.argv[maxArg + 1]) : 0;
const noOcr = process.argv.includes("--no-ocr");

const pkgDir = existsSync(path.join(pkgNodeDir, "edgeparse_wasm.js"))
  ? pkgNodeDir
  : pkgWebDir;
const entry = path.join(pkgDir, "edgeparse_wasm.js");
if (!existsSync(entry)) {
  console.error(
    "Missing wasm pkg. Run: make wasm-build-node\n" +
      "  (or: wasm-pack build crates/edgeparse-wasm --target nodejs --release --out-dir pkg-node)",
  );
  process.exit(1);
}

mkdirSync(outDir, { recursive: true });

/** @type {import('../../sdks/web/src/ocr/types.ts').OcrBackend | null} */
let ocrBackend = null;
/** @type {{ dispose: Function } | null} */
let ppocrBridge = null;

function sleepSyncMs(ms) {
  const sab = new SharedArrayBuffer(4);
  const lock = new Int32Array(sab);
  Atomics.wait(lock, 0, 0, ms);
}

async function tryInitPpocrHost() {
  if (noOcr) return null;
  const ppuEntry = path.join(odlNodeModules, "ppu-paddle-ocr/package.json");
  if (!existsSync(ppuEntry)) {
    console.warn("ppu-paddle-ocr not installed — OCR degraded");
    return null;
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

  console.log("PP-OCRv6_small ready for SDK Node adapter");

  ppocrBridge = {
    dispose() {
      worker.terminate();
    },
  };

  return {
    name: "ppu-paddle-ocr",
    async ready() {},
    async recognize(req) {
      const { port1, port2 } = new MessageChannel();
      const copy = req.gray instanceof Uint8Array ? req.gray.slice() : new Uint8Array(req.gray);
      worker.postMessage(
        { type: "ocr", gray: copy, width: req.width, height: req.height, port: port2 },
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
      return [];
    },
  };
}

try {
  ocrBackend = await tryInitPpocrHost();
} catch (err) {
  console.warn("PP-OCR init failed:", err?.message || err);
}

// Dynamic import of the SDK node adapter (built JS preferred, TS via node --experimental).
let parsePdfNode;
const distNode = path.join(root, "sdks/web/dist/node/index.js");
if (existsSync(distNode)) {
  ({ parsePdfNode } = await import(pathToFileURL(distNode).href));
} else {
  // Inline minimal two-phase driver matching the SDK contract.
  const mod = await import(pathToFileURL(entry).href);
  if (typeof mod.default === "function") await mod.default();

  parsePdfNode = async (pdfBytes, opts = {}) => {
    const format = opts.format ?? "markdown";
    if (typeof mod.ParseSession === "undefined") {
      const md = mod.convert_to_string(
        pdfBytes,
        format,
        opts.pages,
        opts.readingOrder,
        opts.tableMethod,
      );
      return {
        markdown: md,
        warnings: ["ParseSession missing"],
        quality: "degraded",
        meta: {
          wasmVersion: mod.version?.() ?? "unknown",
          models: [],
          timingsMs: {},
          imagesTotal: 0,
          imagesOcred: 0,
          cacheHits: 0,
          backend: "node",
          tier: "off",
        },
      };
    }
    const session = mod.ParseSession.open(
      pdfBytes,
      {
        pages: opts.pages,
        readingOrder: opts.readingOrder,
        tableMethod: opts.tableMethod,
        fileName: opts.fileName ?? "document.pdf",
      },
      null,
    );
    const candidates = session.candidates();
    let imagesOcred = 0;
    const warnings = [];
    for (const cand of candidates) {
      let words = [];
      if (opts.ocr) {
        try {
          words = await opts.ocr.recognize({
            id: String(cand.id),
            hash: cand.hash,
            width: cand.width,
            height: cand.height,
            gray: session.candidate_gray(cand.id),
          });
          if (words.length) imagesOcred += 1;
        } catch (e) {
          warnings.push(String(e?.message || e));
        }
      }
      session.provide_ocr(cand.id, words);
    }
    const markdown = session.finish(format);
    return {
      markdown,
      warnings,
      quality: candidates.length && !imagesOcred ? "degraded" : "full",
      meta: {
        wasmVersion: mod.version?.() ?? "unknown",
        models: [],
        timingsMs: {},
        imagesTotal: candidates.length,
        imagesOcred,
        cacheHits: 0,
        backend: "node",
        tier: "off",
      },
    };
  };
}

let pdfs = readdirSync(pdfDir).filter((f) => f.endsWith(".pdf")).sort();
if (maxDocs > 0) pdfs = pdfs.slice(0, maxDocs);

let ok = 0;
let fail = 0;
const t0 = performance.now();
const failures = [];
const metaPath = path.join(root, "odl-bench/prediction/edgeparse_wasm/sdk_meta.jsonl");
const metaLines = [];

for (let i = 0; i < pdfs.length; i++) {
  const name = pdfs[i];
  const id = name.replace(/\.pdf$/i, "");
  const bytes = new Uint8Array(readFileSync(path.join(pdfDir, name)));
  const tDoc = performance.now();
  try {
    const result = await parsePdfNode(bytes, {
      format: "markdown",
      tableMethod: "cluster",
      fileName: name,
      wasmModule: pathToFileURL(entry).href,
      ocr: ocrBackend ?? undefined,
    });
    writeFileSync(path.join(outDir, `${id}.md`), result.markdown ?? "");
    metaLines.push(
      JSON.stringify({
        id,
        quality: result.quality,
        meta: result.meta,
        warnings: result.warnings,
        ms: performance.now() - tDoc,
      }),
    );
    ok += 1;
  } catch (err) {
    fail += 1;
    failures.push({ id, error: String(err?.message || err) });
    console.error(`FAIL ${id}:`, err?.message || err);
  }
  if ((i + 1) % 20 === 0 || i + 1 === pdfs.length) {
    console.log(`… ${i + 1}/${pdfs.length} (ok=${ok} fail=${fail})`);
  }
}

writeFileSync(metaPath, metaLines.join("\n") + "\n");
ppocrBridge?.dispose?.();

console.log(
  `SDK WASM bench done: ok=${ok} fail=${fail} in ${((performance.now() - t0) / 1000).toFixed(1)}s`,
);
console.log(`Wrote markdown → ${outDir}`);
console.log(`Wrote meta → ${metaPath}`);
if (fail) process.exit(1);

// Silence unused in case of older node
void sdkNode;
