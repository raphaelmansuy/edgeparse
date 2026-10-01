#!/usr/bin/env node
/**
 * Fail closed if models.json still has placeholder (all-zero) sha256 hashes.
 * Used by release-wasm.yml before npm publish.
 */
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(readFileSync(join(root, 'models/models.json'), 'utf8'));
const bad = (manifest.models ?? []).filter((m) => /^0+$/.test(m.sha256 ?? ''));
if (bad.length) {
  console.error(
    'Refuse to publish: placeholder sha256 in models.json for:',
    bad.map((m) => m.id).join(', '),
  );
  process.exit(1);
}
console.log(`models.json OK — ${manifest.models.length} models with real sha256`);
