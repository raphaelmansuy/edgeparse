#!/usr/bin/env node
/** Fail CI if the published ESM bundle exceeds the size budget. */
import { readdir, stat } from 'node:fs/promises';
import { join } from 'node:path';

const BUDGET_BYTES = 250_000; // JS only (workers + client), excl. wasm / onnx
const DIST = new URL('../dist/', import.meta.url);

async function totalJs(dir) {
  let sum = 0;
  const entries = await readdir(dir, { withFileTypes: true });
  for (const e of entries) {
    const p = join(dir, e.name);
    if (e.isDirectory()) sum += await totalJs(p);
    else if (e.name.endsWith('.js')) sum += (await stat(p)).size;
  }
  return sum;
}

const bytes = await totalJs(DIST.pathname);
const kb = (bytes / 1024).toFixed(1);
const budgetKb = (BUDGET_BYTES / 1024).toFixed(0);
console.log(`@edgeparse/web JS bundle: ${kb} KiB (budget ${budgetKb} KiB)`);
if (bytes > BUDGET_BYTES) {
  console.error('SIZE BUDGET EXCEEDED');
  process.exit(1);
}
