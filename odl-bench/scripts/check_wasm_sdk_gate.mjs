#!/usr/bin/env node
/**
 * CI gate: odl-bench evaluation.json must hold TEDS >= 0.776 and overall >= 0.854.
 *
 *   node odl-bench/scripts/check_wasm_sdk_gate.mjs [path/to/evaluation.json]
 */
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const DEFAULT = path.join(
  root,
  "odl-bench/prediction/edgeparse_wasm/evaluation.json",
);
const FALLBACK = path.join(
  root,
  "odl-bench/prediction/edgeparse/evaluation.json",
);

const MIN_TEDS = 0.776;
const MIN_OVERALL = 0.854;
/** Float tolerance — published board rounds to 3 decimals. */
const EPS = 5e-4;

const target = process.argv[2] || (existsSync(DEFAULT) ? DEFAULT : FALLBACK);
if (!existsSync(target)) {
  console.error(`Missing evaluation file: ${target}`);
  process.exit(1);
}

const data = JSON.parse(readFileSync(target, "utf8"));
const score = data.metrics?.score ?? data.mean ?? data.summary ?? {};
const teds = Number(
  score.teds_mean ?? score.TEDS ?? score.teds ?? data.metrics?.teds_mean ?? NaN,
);
const overall = Number(
  score.overall_mean ??
    score.overall ??
    score.Overall ??
    data.metrics?.overall_mean ??
    NaN,
);

console.log(`Gate file: ${target}`);
console.log(`TEDS=${teds} (min ${MIN_TEDS}, eps ${EPS})`);
console.log(`overall=${overall} (min ${MIN_OVERALL}, eps ${EPS})`);

let failed = false;
if (!Number.isFinite(teds) || teds + EPS < MIN_TEDS) {
  console.error(`TEDS gate FAILED`);
  failed = true;
}
if (!Number.isFinite(overall) || overall + EPS < MIN_OVERALL) {
  console.error(`overall gate FAILED`);
  failed = true;
}
if (failed) process.exit(1);
console.log("SDK quality gates PASSED");
