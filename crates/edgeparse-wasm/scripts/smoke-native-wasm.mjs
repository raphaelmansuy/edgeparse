#!/usr/bin/env node
/**
 * Smoke: WASM markdown must match native CLI for a fixture PDF.
 * Usage: node smoke-native-wasm.mjs <pdf> [native-bin] [wasm-pkg]
 */
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(__dirname, "../../..");
const pdf = process.argv[2];
const nativeBin =
  process.argv[3] || path.join(repo, "target/release/edgeparse");
const wasmPkg =
  process.argv[4] || path.join(repo, "crates/edgeparse-wasm/pkg-node");

if (!pdf || !fs.existsSync(pdf)) {
  console.error("usage: smoke-native-wasm.mjs <pdf>");
  process.exit(2);
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "ep-smoke-"));
const env = { ...process.env, EDGEPARSE_RASTER_TABLE_OCR: "off" };
const r = spawnSync(
  nativeBin,
  [pdf, "-o", tmp, "-f", "markdown", "--image-output", "off", "-q"],
  { env, encoding: "utf8" },
);
if (r.status !== 0) {
  console.error(r.stderr);
  process.exit(1);
}
const stem = path.basename(pdf, path.extname(pdf));
const nativeMd = fs.readFileSync(path.join(tmp, `${stem}.md`), "utf8");

const require = createRequire(import.meta.url);
const w = require(path.join(wasmPkg, "edgeparse_wasm.js"));
const bytes = new Uint8Array(fs.readFileSync(pdf));
const wasmMd = w.convert_to_string(bytes, "markdown", undefined, undefined, undefined);
const wasmJson = w.convert_to_string(bytes, "json", undefined, undefined, undefined);

const identical = nativeMd === wasmMd;
const jsonOk = wasmJson.length < 2_000_000 && wasmJson.includes('"type"');
console.log(
  JSON.stringify(
    {
      identical,
      native_chars: nativeMd.length,
      wasm_chars: wasmMd.length,
      json_bytes: wasmJson.length,
      json_compact: jsonOk,
      version: w.version(),
    },
    null,
    2,
  ),
);
process.exit(identical && jsonOk ? 0 : 1);
