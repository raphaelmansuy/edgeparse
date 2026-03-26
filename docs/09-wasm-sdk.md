# EdgeParse WebAssembly SDK

## Objectives

The EdgeParse WASM SDK brings the full Rust-native PDF extraction engine directly into the browser. No server round-trips, no file uploads to third-party services, no backend infrastructure required.

**Primary goals:**

1. **Client-side PDF parsing** — extract text, tables, headings, and structure from PDFs entirely in the browser
2. **Zero-latency extraction** — no network calls; parsing runs locally in the user's browser tab
3. **Privacy by design** — PDF data never leaves the user's device
4. **Universal deployment** — works in any modern browser (Chrome, Firefox, Safari, Edge) via standard WebAssembly

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

## API Reference

The WASM package exports three functions:

### `convert(pdfBytes, format?, pages?, readingOrder?, tableMethod?)`

Parses PDF bytes and returns a structured JavaScript object (the full `PdfDocument` model with pages, elements, bounding boxes).

```typescript
import init, { convert } from '@edgeparse/edgeparse-wasm';

await init(); // Load WASM binary (once)

const response = await fetch('/my-report.pdf');
const bytes = new Uint8Array(await response.arrayBuffer());

const doc = convert(bytes, 'json');
// doc.pages[0].elements → [{type: "heading", text: "...", bbox: {...}}, ...]
```

### `convert_to_string(pdfBytes, format?, pages?, readingOrder?, tableMethod?)`

Parses PDF bytes and returns a formatted string output.

```typescript
import init, { convert_to_string } from '@edgeparse/edgeparse-wasm';

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
import { version } from '@edgeparse/edgeparse-wasm';
console.log(version()); // "0.2.1"
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `pdfBytes` | `Uint8Array` | (required) | Raw PDF file bytes |
| `format` | `string \| null` | `"json"` | `"json"`, `"markdown"`, `"html"`, `"text"` |
| `pages` | `string \| null` | `"all"` | Page range: `"all"`, `"1-5"`, `"1,3,7"` |
| `readingOrder` | `string \| null` | `"auto"` | `"auto"` (XY-Cut++) or `"off"` |
| `tableMethod` | `string \| null` | `"default"` | `"default"` (ruling lines) or `"cluster"` (borderless) |

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

```typescript
// In your service worker
const CACHE_NAME = 'edgeparse-v1';
const WASM_URL = '/edgeparse_wasm_bg.wasm';

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then(cache => cache.add(WASM_URL))
  );
});
```

### 4. Privacy-sensitive document handling

Process confidential documents (medical records, legal contracts, financial statements) without sending data to any server. The PDF never leaves the browser tab.

### 5. Static site document tools

Deploy PDF conversion tools on static hosting (GitHub Pages, Netlify, Vercel) with zero backend costs. The entire application is client-side JavaScript + WASM.

### 6. Browser extension for PDF extraction

Build a Chrome/Firefox extension that extracts structured content from any PDF the user opens, adding copy-as-Markdown or export-to-JSON functionality.

### 7. Embedded PDF processing in SaaS products

Add PDF extraction as a feature in your web application without provisioning additional backend compute. Each user's browser handles its own PDF processing.

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

### Use in your project

```bash
# Option 1: Link locally
npm install ./path-to/crates/edgeparse-wasm/pkg

# Option 2: Copy the pkg/ contents into your project
cp -r crates/edgeparse-wasm/pkg/ my-app/src/edgeparse-wasm/
```

### Vite configuration

```typescript
// vite.config.ts
import { defineConfig } from 'vite';

export default defineConfig({
  optimizeDeps: {
    exclude: ['@edgeparse/edgeparse-wasm'],
  },
  build: {
    target: 'esnext',
  },
});
```

### Webpack configuration

```javascript
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
};
```

## Live Demo

Try EdgeParse WASM in your browser: [edgeparse.com/demo/](https://edgeparse.com/demo/)

The demo lets you:
- Upload or drag-and-drop any PDF
- View extracted content in Markdown, HTML, JSON, or plain text
- Preview rendered Markdown output
- See per-page PDF rendering alongside extracted content
- All processing happens locally — no server, no uploads
