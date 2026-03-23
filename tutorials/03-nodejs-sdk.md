# Tutorial 03 — Node.js SDK

**Goal:** Install `edgeparse`, convert PDFs from TypeScript or JavaScript, and use the `npx edgeparse` CLI.

→ **Previous:** [Python SDK](02-python-sdk.md) · **Next:** [Rust library](04-rust-library.md)

---

## Table of Contents

1. [Installation](#1-installation)
2. [Quick start (ESM)](#2-quick-start-esm)
3. [Quick start (CJS / CommonJS)](#3-quick-start-cjs--commonjs)
4. [TypeScript setup](#4-typescript-setup)
5. [Convert to every format](#5-convert-to-every-format)
6. [Page ranges](#6-page-ranges)
7. [Table detection methods](#7-table-detection-methods)
8. [Image extraction](#8-image-extraction)
9. [Encrypted PDFs](#9-encrypted-pdfs)
10. [Batch processing](#10-batch-processing)
11. [Parse the JSON output](#11-parse-the-json-output)
12. [Error handling](#12-error-handling)
13. [Using the CLI (npx)](#13-using-the-cli-npx)
14. [Build from source](#14-build-from-source)
15. [API reference](#15-api-reference)

---

## 1. Installation

```bash
npm install edgeparse
```

Requires Node.js 18+. Pre-built native addons are bundled as optional dependencies for:
- `edgeparse-darwin-arm64` — macOS Apple Silicon
- `edgeparse-darwin-x64` — macOS Intel
- `edgeparse-linux-x64-gnu` — Linux x64 (glibc 2.31+)
- `edgeparse-linux-arm64-gnu` — Linux arm64
- `edgeparse-win32-x64-msvc` — Windows x64

npm auto-selects the correct addon for your platform when you install.

Verify:

```bash
node -e "const { version } = require('edgeparse'); console.log(version());"
# 0.1.0
```

---

## 2. Quick Start (ESM)

```js
// convert.mjs
import { convert, version } from 'edgeparse';

console.log('edgeparse', version());

const markdown = convert('examples/pdf/lorem.pdf', { format: 'markdown' });
console.log(markdown.slice(0, 200));
```

```bash
node convert.mjs
```

> Note: The package ships CJS (`.js`) and ESM (`.mjs`) builds. ESM import works in Node.js 18+ with `"type": "module"` set in your `package.json`, or in `.mjs` files.

---

## 3. Quick Start (CJS / CommonJS)

```js
// convert.js
const { convert, version } = require('edgeparse');

console.log('edgeparse', version());

const markdown = convert('examples/pdf/lorem.pdf', { format: 'markdown' });
console.log(markdown.slice(0, 200));
```

```bash
node convert.js
```

---

## 4. TypeScript Setup

Install the package — types are bundled:

```bash
npm install edgeparse
```

`tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "esModuleInterop": true,
    "strict": true
  }
}
```

Usage:

```ts
// convert.ts
import { convert, version } from 'edgeparse';
import type { ConvertOptions } from 'edgeparse';

const options: ConvertOptions = {
  format: 'markdown',
  pages: '1-3',
  tableMethod: 'cluster',
};

const markdown: string = convert('report.pdf', options);
console.log(markdown);
```

Compile and run:

```bash
npx tsc convert.ts --esModuleInterop --module commonjs
node convert.js
```

Or use `ts-node`:

```bash
npx ts-node convert.ts
```

---

## 5. Convert to Every Format

### Markdown (default)

```js
const { convert } = require('edgeparse');

const md = convert('report.pdf', { format: 'markdown' });
// Returns Markdown string with GFM tables and headings
```

### JSON with bounding boxes

```js
const raw = convert('report.pdf', { format: 'json' });
const doc = JSON.parse(raw);

console.log(doc['file name']);       // "report.pdf"
console.log(doc['number of pages']); // integer
doc.kids.slice(0, 3).forEach(el => {
  console.log(el.type, el.content.slice(0, 40));
});
```

### HTML

```js
const html = convert('report.pdf', { format: 'html' });
// Returns a complete <!DOCTYPE html> document
```

### Plain text

```js
const text = convert('report.pdf', { format: 'text' });
// UTF-8 text, reading order preserved, no markup
```

---

## 6. Page Ranges

```js
// Pages 1 and 2
const md = convert('paper.pdf', { format: 'markdown', pages: '1-2' });

// Pages 1, 3, and 5 through 7
const md2 = convert('paper.pdf', { format: 'markdown', pages: '1,3,5-7' });

// Just the first page
const first = convert('paper.pdf', { format: 'markdown', pages: '1' });
```

Pages are 1-indexed. Out-of-range pages are silently ignored.

---

## 7. Table Detection Methods

```js
// Default: ruling-line detection (PDFs with visible table borders)
const md = convert('report.pdf', {
  format: 'markdown',
  tableMethod: 'default',
});

// Cluster: geometric detection (borderless tables)
const md2 = convert('report.pdf', {
  format: 'markdown',
  tableMethod: 'cluster',
});
```

---

## 8. Image Extraction

```js
// Off (default) — no image data
const md = convert('doc.pdf', { format: 'markdown', imageOutput: 'off' });

// Embedded — base64 data URIs in the output
const mdEmbedded = convert('doc.pdf', {
  format: 'markdown',
  imageOutput: 'embedded',
});
```

> The `'external'` mode saves images to the filesystem. When using the Node.js SDK for external images, use the CLI for a filepath-based workflow: `npx edgeparse doc.pdf --image-output external --image-dir ./images -o output/`.

---

## 9. Encrypted PDFs

```js
const md = convert('secure.pdf', {
  format: 'markdown',
  password: 'my-secret-password',
});
```

---

## 10. Batch Processing

### Sequential

```js
const { convert } = require('edgeparse');
const fs = require('fs');
const path = require('path');

const pdfDir = 'pdfs/';
const outDir = 'output/';
fs.mkdirSync(outDir, { recursive: true });

for (const file of fs.readdirSync(pdfDir).filter(f => f.endsWith('.pdf'))) {
  const inputPath = path.join(pdfDir, file);
  const stem = path.basename(file, '.pdf');
  const outPath = path.join(outDir, `${stem}.md`);

  const md = convert(inputPath, { format: 'markdown' });
  fs.writeFileSync(outPath, md, 'utf-8');
  console.log(`✓ ${file} → ${outPath}`);
}
```

### Parallel with `Promise.all`

Convert is synchronous (the native addon is CPU-bound), so true async parallelism requires worker threads:

```js
const { convert } = require('edgeparse');
const { Worker, isMainThread, parentPort, workerData } = require('worker_threads');
const fs = require('fs');
const path = require('path');

// worker.js
if (!isMainThread) {
  const { inputPath, format } = workerData;
  const result = convert(inputPath, { format });
  parentPort.postMessage(result);
}

// main
async function processParallel(pdfFiles) {
  return Promise.all(
    pdfFiles.map(file =>
      new Promise((resolve, reject) => {
        const worker = new Worker(__filename, {
          workerData: { inputPath: file, format: 'markdown' },
        });
        worker.once('message', resolve);
        worker.once('error', reject);
      })
    )
  );
}
```

> For most use cases, the Rust engine's internal Rayon parallelism already saturates available cores. Sequential calling from Node.js is sufficient.

### Write to files

```js
const { convert } = require('edgeparse');
const fs = require('fs');
const path = require('path');

function convertToFile(inputPath, outDir, format = 'markdown') {
  const ext = { markdown: 'md', json: 'json', html: 'html', text: 'txt' }[format] ?? 'md';
  const stem = path.basename(inputPath, '.pdf');
  const outPath = path.join(outDir, `${stem}.${ext}`);
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(outPath, convert(inputPath, { format }), 'utf-8');
  return outPath;
}

const out = convertToFile('report.pdf', 'output/', 'markdown');
console.log('Written to', out);
```

---

## 11. Parse the JSON Output

```js
const { convert } = require('edgeparse');

const raw = convert('examples/pdf/1901.03003.pdf', { format: 'json' });
const doc = JSON.parse(raw);

// Document metadata
console.log('Title:', doc.title ?? '(none)');
console.log('Author:', doc.author);
console.log('Pages:', doc['number of pages']);

// All headings
const headings = doc.kids.filter(el => el.type === 'heading');
headings.slice(0, 5).forEach(h => {
  console.log(`  H${h.level ?? '?'} — ${h.content}`);
});

// Elements on page 1
const page1 = doc.kids.filter(el => el['page number'] === 1);
console.log(`\n${page1.length} elements on page 1`);

// Bounding boxes — coordinates are [x0, y0, x1, y1] in PDF points (72 pt/inch)
page1.slice(0, 3).forEach(el => {
  const [x0, y0, x1, y1] = el['bounding box'];
  console.log(`  ${el.type} at (${x0.toFixed(0)}, ${y0.toFixed(0)}) — ${el.content.slice(0, 40)}`);
});
```

See [Tutorial 05 — Output Formats](05-output-formats.md#json) for the full JSON schema.

---

## 12. Error Handling

```js
const { convert } = require('edgeparse');

// File not found
try {
  convert('/nonexistent.pdf');
} catch (err) {
  console.error(err.message); // "File not found: /nonexistent.pdf"
}

// Invalid format
try {
  convert('doc.pdf', { format: 'xyz' });
} catch (err) {
  console.error(err.message); // "Unknown format: xyz"
}

// Wrong password
try {
  convert('secure.pdf', { password: 'wrong' });
} catch (err) {
  console.error(err.message);
}
```

All errors are thrown as standard JavaScript `Error` objects with a descriptive `message`.

---

## 13. Using the CLI (npx)

The package installs an `edgeparse` binary:

```bash
# Convert to Markdown, print to stdout
npx edgeparse examples/pdf/lorem.pdf -f markdown

# Convert and write to a file
npx edgeparse examples/pdf/lorem.pdf -f markdown -o output/lorem.md

# JSON output
npx edgeparse examples/pdf/lorem.pdf -f json -o output/lorem.json

# Page range
npx edgeparse paper.pdf -f markdown -p "1-3" -o output/paper.md

# Show help
npx edgeparse --help

# Show version
npx edgeparse --version
```

Full CLI options:

```
Options:
  -f, --format <fmt>         Output format: markdown, json, html, text (default: markdown)
  -p, --pages <range>        Page range, e.g. "1,3,5-7"
      --password <pw>        Password for encrypted PDFs
      --reading-order <algo> xycut (default) or off
      --table-method <m>     default or cluster
      --image-output <mode>  off (default), embedded, or external
  -o, --output <path>        Output file path (default: stdout)
  -v, --version              Show version
  -h, --help                 Show this help
```

---

## 14. Build from Source

If a pre-built addon is not available for your platform:

```bash
git clone https://github.com/raphaelmansuy/edgeparse.git
cd edgeparse

# Build the native addon (requires Rust 1.85+)
cargo build --release -p edgeparse-node

# Build the TypeScript wrapper
cd sdks/node
npm install
npm run build

# Link the addon for local testing
mkdir -p node_modules/edgeparse-darwin-arm64   # Adjust platform name
cp ../../target/release/libedgeparse_node.dylib \
   node_modules/edgeparse-darwin-arm64/edgeparse-node.darwin-arm64.node
cp npm/darwin-arm64/package.json \
   node_modules/edgeparse-darwin-arm64/
```

Test:

```bash
node -e "const { version } = require('./dist/index.js'); console.log(version());"
# 0.1.0
```

Platform-to-filename mapping:

| Platform | Library file | Addon filename |
|---------|-------------|----------------|
| macOS arm64 | `libedgeparse_node.dylib` | `edgeparse-node.darwin-arm64.node` |
| macOS x64 | `libedgeparse_node.dylib` | `edgeparse-node.darwin-x64.node` |
| Linux x64 | `libedgeparse_node.so` | `edgeparse-node.linux-x64-gnu.node` |
| Linux arm64 | `libedgeparse_node.so` | `edgeparse-node.linux-arm64-gnu.node` |
| Windows x64 | `edgeparse_node.dll` | `edgeparse-node.win32-x64-msvc.node` |

---

## 15. API Reference

### `convert(inputPath, options?)`

```ts
function convert(inputPath: string, options?: ConvertOptions): string
```

Returns the extracted content as a string.

### `ConvertOptions`

```ts
interface ConvertOptions {
  format?: string;        // "markdown" | "json" | "html" | "text"  (default: "markdown")
  pages?: string;         // e.g. "1,3,5-7"
  password?: string;
  readingOrder?: string;  // "xycut" (default) | "off"
  tableMethod?: string;   // "default" | "cluster"
  imageOutput?: string;   // "off" (default) | "embedded" | "external"
}
```

### `version()`

```ts
function version(): string
```

Returns the version string, e.g. `"0.1.0"`.

---

→ **Continue:** [Rust Library Tutorial](04-rust-library.md)
