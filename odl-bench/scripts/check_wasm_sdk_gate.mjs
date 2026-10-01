#!/usr/bin/env node
/**
 * CI gate: odl-bench evaluation must hold TEDS >= 0.776 and overall >= 0.854.
 *
 *   node odl-bench/scripts/check_wasm_sdk_gate.mjs [path/to/evaluation.json]
 *
 * Prefers live prediction/evaluation.json when present; otherwise falls back
 * to the committed summary under odl-bench/reports/.
 */
import { readFileSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");
const LIVE = path.join(
  root,
  "odl-bench/prediction/edgeparse_wasm/evaluation.json",
);
const LIVE_FALLBACK = path.join(
  root,
  "odl-bench/prediction/edgeparse/evaluation.json",
);
const SNAPSHOT = path.join(root, "odl-bench/reports/edgeparse_wasm.summary.json");

const MIN_TEDS = 0.776;
const MIN_OVERALL = 0.854;
/** Float tolerance — published board rounds to 3 decimals. */
const EPS = 5e-4;

function pickTarget() {
  if (process.argv[2]) return process.argv[2];
  if (existsSync(LIVE)) return LIVE;
  if (existsSync(LIVE_FALLBACK)) return LIVE_FALLBACK;
  if (existsSync(SNAPSHOT)) return SNAPSHOT;
  return LIVE;
}

function extractScores(data) {
  // Committed summary shape: { teds, overall, ... }
  if (data.teds != null || data.overall != null) {
    return {
      teds: Number(data.teds ?? data.TEDS ?? NaN),
      overall: Number(data.overall ?? data.Overall ?? NaN),
    };
  }
  const score = data.metrics?.score ?? data.mean ?? data.summary ?? {};
  const metrics = data.metrics ?? {};
  return {
    teds: Number(
      score.teds_mean ?? score.TEDS ?? score.teds ?? metrics.teds_mean ?? NaN,
    ),
    overall: Number(
      score.overall_mean ??
        score.overall ??
        score.Overall ??
        metrics.overall_mean ??
        NaN,
    ),
  };
}

const target = pickTarget();
if (!existsSync(target)) {
  console.error(`Missing evaluation file: ${target}`);
  process.exit(1);
}

const data = JSON.parse(readFileSync(target, "utf8"));
const { teds, overall } = extractScores(data);

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
