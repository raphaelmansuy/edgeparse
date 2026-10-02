#!/usr/bin/env node
/**
 * Assert pattern-painted page rasters surface OCR candidates and skip
 * artifact metadata titles (Skia/Penpot "… - Render").
 *
 *   wasm-pack build crates/edgeparse-wasm --target web --out-dir pkg
 *   node crates/edgeparse-wasm/scripts/e2e-pattern-form.mjs
 */
import { existsSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../../..");
const pkgDir = process.env.EDGEPARSE_WASM_DIR || path.join(root, "crates/edgeparse-wasm/pkg");
const entry = path.join(pkgDir, "edgeparse_wasm.js");
const wasmBin = path.join(pkgDir, "edgeparse_wasm_bg.wasm");
const pdfPath =
  process.env.EDGEPARSE_E2E_PDF ||
  path.join(root, "crates/edgeparse-wasm/testdata/pattern_form.pdf");

if (!existsSync(entry) || !existsSync(wasmBin)) {
  console.log(
    "SKIP: wasm pkg not built (run: wasm-pack build crates/edgeparse-wasm --target web --out-dir pkg).",
  );
  process.exit(0);
}
if (!existsSync(pdfPath)) {
  console.error("missing PDF fixture:", pdfPath);
  process.exit(1);
}

const mod = await import(pathToFileURL(entry).href);
await mod.default({ module_or_path: readFileSync(wasmBin) });

const pdf = new Uint8Array(readFileSync(pdfPath));
const session = mod.ParseSession.open(
  pdf,
  { fileName: "pattern_form.pdf", tableMethod: "cluster" },
  null,
);
const candidates = session.candidates();
const candList = Array.isArray(candidates)
  ? candidates
  : candidates && typeof candidates === "object"
    ? Object.values(candidates)
    : [];
if (candList.length < 1) {
  throw new Error(`expected ≥1 OCR candidate, got ${candList.length}`);
}
const kinds = candList.map((c) => c?.kind ?? c?.Kind ?? "(none)");
console.log("candidates=", candList.length, "kinds=", kinds.join(","));

const all = session.finishAll();
const get = (k) => (all instanceof Map ? all.get(k) : all[k]);
const md = get("markdown") ?? "";
if (md.includes("# Penpot - Render")) {
  throw new Error(`artifact title leaked into markdown:\n${md}`);
}
if (!/ACME|NAME VALUE/i.test(md)) {
  throw new Error(`expected native fixture text in markdown:\n${md}`);
}
console.log("markdown ok, length=", md.length);
console.log("e2e pattern-form PASS");
