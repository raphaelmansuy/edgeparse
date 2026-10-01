# Content Security Policy for `@edgeparse/web`

EdgeParse runs fully locally after models are cached. Suggested CSP:

```http
Content-Security-Policy:
  default-src 'self';
  script-src 'self' 'wasm-unsafe-eval';
  worker-src 'self' blob:;
  connect-src 'self' https://cdn.jsdelivr.net https://cdn.jsdelivr.net/gh/;
  img-src 'self' data: blob:;
  style-src 'self' 'unsafe-inline';
```

Notes:

- **`script-src 'wasm-unsafe-eval'`** — required for WebAssembly instantiate.
- **`worker-src`** — parse + OCR module workers (and `blob:` if your bundler inlines workers).
- **`connect-src`** — allowlist model CDN hosts from `models/models.json` only.
- **No `eval`** — the SDK never calls `eval` / `new Function`.
- **No COOP/COEP required** — the two-phase OCR path avoids `Atomics.wait`.

Model integrity: every downloaded artifact is sha256-verified before the `.done` marker is written. Placeholder hashes (`0…0`) in early manifests skip verification until pins land.
