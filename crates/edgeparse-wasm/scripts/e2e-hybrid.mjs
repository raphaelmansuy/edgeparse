#!/usr/bin/env node
/**
 * E2E smoke for convert + convert_hybrid against wasm-pack nodejs output.
 *
 *   wasm-pack build crates/edgeparse-wasm --target nodejs --out-dir pkg
 *   node crates/edgeparse-wasm/scripts/e2e-hybrid.mjs
 */
import { existsSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../../..");
const pkgDir = process.env.EDGEPARSE_WASM_DIR || path.join(root, "crates/edgeparse-wasm/pkg");
const entry = path.join(pkgDir, "edgeparse_wasm.js");
const pdfPath =
  process.env.EDGEPARSE_E2E_PDF ||
  path.join(root, "crates/edgeparse-wasm/testdata/hello.pdf");

if (!existsSync(entry)) {
  console.log(
    "SKIP: wasm pkg not built (run: wasm-pack build crates/edgeparse-wasm --target nodejs --out-dir pkg).",
  );
  process.exit(0);
}
if (!existsSync(pdfPath)) {
  console.error("missing PDF fixture:", pdfPath);
  process.exit(1);
}

const pdf = readFileSync(pdfPath);
const mod = await import(pathToFileURL(entry).href);
if (typeof mod.default === "function") {
  await mod.default();
} else if (mod.init) {
  await mod.init();
}

const md = mod.convert_to_string(pdf, "markdown", null, null, null);
if (typeof md !== "string") {
  throw new Error("convert_to_string did not return a string");
}
console.log("convert_to_string ok, length=", md.length);

const backend = "| A | B |\n| --- | --- |\n| 1 | 2 |\n";
const hybrid = mod.convert_hybrid(pdf, backend, "markdown", null, null, null);
if (typeof hybrid !== "string") {
  throw new Error("convert_hybrid did not return a string");
}
console.log("convert_hybrid ok, length=", hybrid.length);
if (typeof mod.version === "function") {
  console.log("version=", mod.version());
}
console.log("e2e hybrid PASS");
