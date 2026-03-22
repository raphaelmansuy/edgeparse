#!/usr/bin/env node

import { parseArgs } from 'node:util';
import { writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';
import { convert, version } from './index.js';

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    format:        { type: 'string', short: 'f', default: 'markdown' },
    pages:         { type: 'string', short: 'p' },
    password:      { type: 'string' },
    'reading-order': { type: 'string', default: 'xycut' },
    'table-method':  { type: 'string', default: 'default' },
    'image-output':  { type: 'string', default: 'off' },
    output:        { type: 'string', short: 'o' },
    version:       { type: 'boolean', short: 'v' },
    help:          { type: 'boolean', short: 'h' },
  },
});

if (values.version) {
  console.log(`edgeparse ${version()}`);
  process.exit(0);
}

if (values.help || positionals.length === 0) {
  console.log(`\
Usage: edgeparse [options] <input.pdf>

Options:
  -f, --format <fmt>         Output format: markdown, json, html, text (default: markdown)
  -p, --pages <range>        Page range, e.g. "1,3,5-7"
      --password <pw>        Password for encrypted PDFs
      --reading-order <algo> Reading order: xycut (default) or off
      --table-method <m>     Table method: default or cluster
      --image-output <mode>  Image output: off (default), embedded, external
  -o, --output <path>        Output file path (default: stdout)
  -v, --version              Show version
  -h, --help                 Show this help
`);
  process.exit(values.help ? 0 : 1);
}

const inputPath = positionals[0];

try {
  const result = convert(inputPath, {
    format: values.format,
    pages: values.pages,
    password: values.password,
    readingOrder: values['reading-order'],
    tableMethod: values['table-method'],
    imageOutput: values['image-output'],
  });

  if (values.output) {
    writeFileSync(values.output, result, 'utf-8');
  } else {
    process.stdout.write(result);
  }
} catch (err: unknown) {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`edgeparse: ${msg}`);
  process.exit(1);
}
