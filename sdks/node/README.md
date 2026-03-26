# edgeparse

> High-performance PDF extraction for Node.js — Rust engine, JavaScript/TypeScript interface.

[![npm version](https://img.shields.io/npm/v/edgeparse.svg)](https://www.npmjs.com/package/edgeparse)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/raphaelmansuy/edgeparse/blob/main/LICENSE)
[![GitHub](https://img.shields.io/badge/GitHub-raphaelmansuy%2Fedgeparse-181717?logo=github)](https://github.com/raphaelmansuy/edgeparse)

EdgeParse converts PDF documents to Markdown, JSON, HTML, or plain text. It is powered by a native Rust engine (via N-API) with pre-built binaries — no compilation required.

## Install

```bash
npm install edgeparse
# or
pnpm add edgeparse
# or
yarn add edgeparse
```

Pre-built binaries are available for:

| Platform | Architecture |
|---|---|
| macOS | x64, arm64 (Apple Silicon) |
| Linux | x64-gnu, arm64-gnu |
| Windows | x64-msvc |

## Quick Start

```typescript
import { convert } from 'edgeparse';

// Convert a PDF to Markdown
const markdown = convert('report.pdf');
console.log(markdown);

// Convert to JSON
const json = convert('report.pdf', { format: 'json' });

// Convert specific pages to HTML
const html = convert('report.pdf', {
  format: 'html',
  pages: [0, 1, 2],   // pages 1–3 (0-indexed)
});

// Password-protected PDF
const text = convert('secure.pdf', {
  format: 'markdown',
  password: 'secret',
});
```

## API

### `convert(inputPath, options?): string`

Converts a PDF file and returns the content as a string.

| Parameter | Type | Description |
|---|---|---|
| `inputPath` | `string` | Absolute or relative path to the PDF file |
| `options.format` | `'markdown' \| 'json' \| 'html' \| 'text'` | Output format (default: `'markdown'`) |
| `options.pages` | `number[]` | Zero-indexed page numbers to extract (default: all) |
| `options.password` | `string` | Password for encrypted PDFs |
| `options.readingOrder` | `'xycut' \| 'default'` | Reading order algorithm (default: `'xycut'`) |
| `options.tableMethod` | `'border' \| 'cluster'` | Table detection method (default: `'border'`) |
| `options.imageOutput` | `'embedded' \| 'external' \| 'none'` | Image handling (default: `'none'`) |

### `version(): string`

Returns the edgeparse engine version string.

```typescript
import { version } from 'edgeparse';
console.log(version()); // e.g. "0.2.1"
```

## CLI

The package also ships an `edgeparse` CLI binary:

```bash
npx edgeparse document.pdf
npx edgeparse document.pdf --format json
npx edgeparse document.pdf --format html --output output/
```

## TypeScript

Full TypeScript support is included — no `@types` package needed.

```typescript
import { convert, version } from 'edgeparse';
import type { ConvertOptions } from 'edgeparse';
```

## Performance

EdgeParse consistently processes **40+ pages/second** on a modern machine and achieves **88%+ extraction accuracy** on diverse real-world PDFs — dramatically faster than Python-based alternatives.

## Links

- [GitHub](https://github.com/raphaelmansuy/edgeparse)
- [Documentation](https://edgeparse.com)
- [PyPI (Python)](https://pypi.org/project/edgeparse/)
- [crates.io (Rust CLI)](https://crates.io/crates/edgeparse-cli)
- [crates.io (Rust Core)](https://crates.io/crates/edgeparse-core)

## License

Apache-2.0 — see [LICENSE](https://github.com/raphaelmansuy/edgeparse/blob/main/LICENSE).
