# EdgeParse documentation site

Astro + Starlight site for [edgeparse.com](https://edgeparse.com) — product landing page, API docs, guides, and benchmark boards for EdgeParse **v0.3.0**.

## Develop

```bash
pnpm install
pnpm dev        # http://localhost:4321
pnpm build      # production output → dist/
pnpm preview    # preview the production build
```

## Content

| Path | Purpose |
|------|---------|
| `src/content/docs/` | Starlight MDX docs (getting started, API, guides, benchmark) |
| `src/components/landing/` | Splash / marketing sections |
| `src/data/benchmark.ts` | **Single source of truth** for harness + odl-bench numbers (`claims`, `speedMultipleVs`, `leaderFor`) |
| `astro.config.mjs` | Sidebar, SEO / JSON-LD, integrations |

Do not hard-code benchmark figures in pages — import from `src/data/benchmark.ts`.

## Two benchmark boards

1. **EdgeParse harness** (`benchmarkSnapshot`) — product regression board  
2. **Official odl-bench** (`odlBenchSnapshot`) — public OpenDataLoader-formula board  

See `src/content/docs/benchmark/results.mdx`.

## Deploy

GitHub Actions workflow `.github/workflows/deploy-site.yml` builds this package and publishes to the production host.
