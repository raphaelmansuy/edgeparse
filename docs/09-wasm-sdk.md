# EdgeParse WebAssembly SDK

## Objectives

The EdgeParse WASM SDK brings the full Rust-native PDF extraction engine directly into the browser. No server round-trips, no file uploads to third-party services, no backend infrastructure required.

**Primary goals:**

1. **Client-side PDF parsing** — extract text, tables, headings, and structure from PDFs entirely in the browser
2. **Zero-latency extraction** — no network calls; parsing runs locally in the user's browser tab
3. **Privacy by design** — PDF data never leaves the user's device
4. **Universal deployment** — works in any modern browser (Chrome, Firefox, Safari, Edge) via standard WebAssembly

## Distribution

EdgeParse WASM is published to multiple registries and CDNs on every tagged release.

### Primary: npm

The canonical package is `edgeparse-wasm` on the public npm registry.

```bash
npm install edgeparse-wasm
# or
pnpm add edgeparse-wasm
# or
yarn add edgeparse-wasm
```

Package page: <https://www.npmjs.com/package/edgeparse-wasm>

### CDN: jsDelivr

Served automatically from npm. No installation required — useful for prototyping,
sandboxes, and static sites.

```html
<!-- Latest release -->
<script type="module">
  import init, { convert_to_string } from 'https://cdn.jsdelivr.net/npm/edgeparse-wasm/edgeparse_wasm.js';
  await init('https://cdn.jsdelivr.net/npm/edgeparse-wasm/edgeparse_wasm_bg.wasm');
</script>

<!-- Pin to a specific version -->
<script type="module">
  import init, { convert_to_string } from 'https://cdn.jsdelivr.net/npm/edgeparse-wasm@0.2.4/edgeparse_wasm.js';
  await init('https://cdn.jsdelivr.net/npm/edgeparse-wasm@0.2.4/edgeparse_wasm_bg.wasm');
</script>
```

### CDN: unpkg

Alternative CDN also served directly from npm.

```html
<script type="module">
  import init, { convert_to_string } from 'https://unpkg.com/edgeparse-wasm@0.2.4/edgeparse_wasm.js';
  await init('https://unpkg.com/edgeparse-wasm@0.2.4/edgeparse_wasm_bg.wasm');
</script>
```

### Secondary: GitHub Packages

For enterprise or GitHub-native workflows, the package is also published to GitHub
Packages under the scoped name `@raphaelmansuy/edgeparse-wasm`.

**Authenticate first** (read access requires a GitHub token even for public packages):

```bash
# 1. Create a Personal Access Token with read:packages scope
#    https://github.com/settings/tokens

# 2. Add the scoped registry to .npmrc
echo "@raphaelmansuy:registry=https://npm.pkg.github.com" >> .npmrc
echo "//npm.pkg.github.com/:_authToken=YOUR_TOKEN" >> .npmrc

# 3. Install
npm install @raphaelmansuy/edgeparse-wasm
```

Or set the token via an environment variable in CI:

```bash
echo "@raphaelmansuy:registry=https://npm.pkg.github.com" >> .npmrc
echo "//npm.pkg.github.com/:_authToken=${GITHUB_TOKEN}" >> .npmrc
npm install @raphaelmansuy/edgeparse-wasm
```

Package page: <https://github.com/raphaelmansuy/edgeparse/pkgs/npm/edgeparse-wasm>

### Distribution summary

| Registry | Package name | URL |
|----------|-------------|-----|
| npm | `edgeparse-wasm` | <https://www.npmjs.com/package/edgeparse-wasm> |
| jsDelivr CDN | (mirrors npm) | `https://cdn.jsdelivr.net/npm/edgeparse-wasm` |
| unpkg CDN | (mirrors npm) | `https://unpkg.com/edgeparse-wasm` |
| GitHub Packages | `@raphaelmansuy/edgeparse-wasm` | <https://github.com/raphaelmansuy/edgeparse/pkgs/npm/edgeparse-wasm> |
| GitHub Releases | `.tgz` tarball | <https://github.com/raphaelmansuy/edgeparse/releases> |

---

## Advantages

### vs. Server-side parsing

| Factor | Server-side | WASM (client-side) |
|--------|------------|-------------------|
| **Latency** | Network round-trip + queue + processing | Instant (local CPU) |
| **Privacy** | PDF uploaded to server | PDF stays on device |
| **Infrastructure** | Requires backend, scaling, monitoring | Zero infrastructure |
| **Cost** | Compute + bandwidth per request | Free (runs on user hardware) |
| **Offline** | Requires internet | Works offline after initial load |

### vs. JavaScript PDF libraries

| Factor | JS libraries (pdf.js, etc.) | EdgeParse WASM |
|--------|---------------------------|----------------|
| **Table extraction** | None or basic | Ruling-line + cluster method |
| **Heading detection** | None | Numbered + unnumbered hierarchy |
| **Reading order** | Stream order only | XY-Cut++ algorithm |
| **Structured output** | Raw text | JSON, Markdown, HTML, plain text |
| **AI safety filters** | None | Hidden text, off-page, tiny-text, OCG |

### Key properties

- **Same engine** — identical Rust code runs in WASM and native; same accuracy, same output
- **~4 MB** — compressed WASM binary, loaded once and cached by the browser
- **No dependencies** — no Java, no Python, no ML models, no GPU
- **TypeScript types** — full `.d.ts` definitions for IDE autocomplete

---

## API Reference

The WASM package exports three functions:

### `convert(pdfBytes, format?, pages?, readingOrder?, tableMethod?)`

Parses PDF bytes and returns a structured JavaScript object (the full `PdfDocument` model with pages, elements, bounding boxes).

```typescript
import init, { convert } from 'edgeparse-wasm';

await init(); // Load WASM binary (once)

const response = await fetch('/my-report.pdf');
const bytes = new Uint8Array(await response.arrayBuffer());

const doc = convert(bytes, 'json');
// doc.pages[0].elements → [{type: "heading", text: "...", bbox: {...}}, ...]
```

### `convert_to_string(pdfBytes, format?, pages?, readingOrder?, tableMethod?)`

Parses PDF bytes and returns a formatted string output.

```typescript
import init, { convert_to_string } from 'edgeparse-wasm';

await init();

const bytes = new Uint8Array(await fetch('/report.pdf').then(r => r.arrayBuffer()));

// Get Markdown
const markdown = convert_to_string(bytes, 'markdown');

// Get HTML
const html = convert_to_string(bytes, 'html');

// Get plain text
const text = convert_to_string(bytes, 'text');

// Get JSON string
const json = convert_to_string(bytes, 'json');
```

### `version()`

Returns the EdgeParse version string.

```typescript
import { version } from 'edgeparse-wasm';
console.log(version()); // "0.2.4"
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `pdfBytes` | `Uint8Array` | (required) | Raw PDF file bytes |
| `format` | `string \| null` | `"json"` | `"json"`, `"markdown"`, `"html"`, `"text"` |
| `pages` | `string \| null` | `"all"` | Page range: `"all"`, `"1-5"`, `"1,3,7"` |
| `readingOrder` | `string \| null` | `"auto"` | `"auto"` (XY-Cut++) or `"off"` |
| `tableMethod` | `string \| null` | `"default"` | `"default"` (ruling lines) or `"cluster"` (borderless) |

---

## Quick-start Examples

### Vite + React (recommended)

```tsx
// src/App.tsx
import { useRef, useState } from 'react';

// Lazy-import so Vite does not pre-bundle the WASM binary.
async function loadEdgeParse() {
  const { default: init, convert_to_string } = await import('edgeparse-wasm');
  await init();
  return { convert_to_string };
}

export default function App() {
  const [output, setOutput] = useState('');
  const ep = useRef<Awaited<ReturnType<typeof loadEdgeParse>> | null>(null);

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    ep.current ??= await loadEdgeParse();
    const bytes = new Uint8Array(await file.arrayBuffer());
    setOutput(ep.current.convert_to_string(bytes, 'markdown') ?? '');
  };

  return (
    <>
      <input type="file" accept=".pdf" onChange={handleFile} />
      <pre>{output}</pre>
    </>
  );
}
```

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  optimizeDeps: { exclude: ['edgeparse-wasm'] },
  build: { target: 'esnext' },
});
```

### Next.js (App Router)

```tsx
// app/pdf-extract/page.tsx  — client component
'use client';
import { useRef, useState } from 'react';

export default function PdfExtract() {
  const [md, setMd] = useState('');
  const ready = useRef(false);

  const handleFile = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!ready.current) {
      const { default: init } = await import('edgeparse-wasm');
      await init();
      ready.current = true;
    }
    const { convert_to_string } = await import('edgeparse-wasm');
    const bytes = new Uint8Array(await file.arrayBuffer());
    setMd(convert_to_string(bytes, 'markdown') ?? '');
  };

  return (
    <>
      <input type="file" accept=".pdf" onChange={handleFile} />
      <pre style={{ whiteSpace: 'pre-wrap' }}>{md}</pre>
    </>
  );
}
```

```javascript
// next.config.js
/** @type {import('next').NextConfig} */
module.exports = {
  webpack(config) {
    config.experiments = { ...config.experiments, asyncWebAssembly: true };
    return config;
  },
};
```

### Vanilla HTML via CDN (no build tool)

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <title>EdgeParse WASM demo</title>
</head>
<body>
  <input id="pick" type="file" accept=".pdf" />
  <pre id="out"></pre>

  <script type="module">
    import init, { convert_to_string, version }
      from 'https://cdn.jsdelivr.net/npm/edgeparse-wasm@0.2.4/edgeparse_wasm.js';

    // Pass the .wasm binary URL explicitly when loading from a CDN.
    await init('https://cdn.jsdelivr.net/npm/edgeparse-wasm@0.2.4/edgeparse_wasm_bg.wasm');

    console.log('EdgeParse', version());

    document.getElementById('pick').addEventListener('change', async (e) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const bytes = new Uint8Array(await file.arrayBuffer());
      document.getElementById('out').textContent =
        convert_to_string(bytes, 'markdown');
    });
  </script>
</body>
</html>
```

### Webpack 5

```javascript
// webpack.config.js
module.exports = {
  experiments: { asyncWebAssembly: true },
};
```

### Service Worker (PWA — offline support)

```javascript
// sw.js
const CACHE = 'edgeparse-v1';
self.addEventListener('install', event => {
  event.waitUntil(
    caches.open(CACHE).then(cache =>
      cache.addAll([
        '/edgeparse_wasm.js',
        '/edgeparse_wasm_bg.wasm',
      ])
    )
  );
});

self.addEventListener('fetch', event => {
  event.respondWith(
    caches.match(event.request).then(r => r ?? fetch(event.request))
  );
});
```

---

## Use Cases

### 1. Browser-based PDF viewer with structured extraction

Build a web app where users drag-and-drop PDFs and instantly see extracted Markdown, JSON, or HTML — without any server. Ideal for document review tools, note-taking apps, and research assistants.

```typescript
// In your file upload handler
fileInput.addEventListener('change', async (e) => {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;

  const bytes = new Uint8Array(await file.arrayBuffer());
  const markdown = convert_to_string(bytes, 'markdown');
  
  document.getElementById('output')!.textContent = markdown;
});
```

### 2. Client-side RAG preprocessing

Prepare PDF content for retrieval-augmented generation (RAG) pipelines directly in the browser. Extract structured chunks before sending them to an embedding API — only the text leaves the device, never the full PDF.

```typescript
const doc = convert(bytes, 'json');

// Extract chunks for embedding
const chunks = doc.pages.flatMap(page =>
  page.elements
    .filter(el => el.type === 'paragraph' || el.type === 'heading')
    .map(el => ({
      text: el.text,
      page: page.page_number,
      bbox: el.bbox,
    }))
);

// Send only text chunks to your embedding API
const embeddings = await fetch('/api/embed', {
  method: 'POST',
  body: JSON.stringify({ chunks: chunks.map(c => c.text) }),
});
```

### 3. Offline-capable document processing

Build Progressive Web Apps (PWAs) that work without internet. Once the WASM binary is cached by the service worker, PDF extraction works entirely offline.

### 4. Privacy-sensitive document handling

Process confidential documents (medical records, legal contracts, financial statements) without sending data to any server. The PDF never leaves the browser tab.

### 5. Static site document tools

Deploy PDF conversion tools on static hosting (GitHub Pages, Netlify, Vercel) with zero backend costs. The entire application is client-side JavaScript + WASM.

### 6. Browser extension for PDF extraction

Build a Chrome/Firefox extension that extracts structured content from any PDF the user opens, adding copy-as-Markdown or export-to-JSON functionality.

### 7. Embedded PDF processing in SaaS products

Add PDF extraction as a feature in your web application without provisioning additional backend compute. Each user's browser handles its own PDF processing.

---

## Building from Source

### Prerequisites

- [Rust 1.85+](https://rustup.rs/)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

### Build the WASM package

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build the WASM package (output goes to crates/edgeparse-wasm/pkg/)
cd crates/edgeparse-wasm
wasm-pack build --target web --release
```

### Use in your project (local build)

```bash
# Option 1: Install from local path
npm install ./crates/edgeparse-wasm/pkg

# Option 2: Copy the pkg/ contents into your project
cp -r crates/edgeparse-wasm/pkg/ my-app/src/edgeparse-wasm/
```

---

## Live Demo

Try EdgeParse WASM in your browser: [edgeparse.com/demo/](https://edgeparse.com/demo/)

The demo lets you:
- Upload or drag-and-drop any PDF
- View extracted content in Markdown, HTML, JSON, or plain text
- Preview rendered Markdown output
- See per-page PDF rendering alongside extracted content
- All processing happens locally — no server, no uploads

