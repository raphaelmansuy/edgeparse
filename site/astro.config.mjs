// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import sitemap from '@astrojs/sitemap';
import tailwindcss from '@tailwindcss/vite';

const siteUrl = 'https://edgeparse.com';
const fullUrl = siteUrl;
const ogImageUrl = `${fullUrl}/og-image.png`;

export default defineConfig({
	site: siteUrl,
	integrations: [
		sitemap({
			changefreq: 'weekly',
			priority: 0.7,
			lastmod: new Date(),
		}),
		starlight({
			title: 'EdgeParse',
			description: 'High-performance PDF-to-structured-data extraction engine. Rust-native, 10-100× faster than alternatives. Python, Node.js, CLI & Rust SDKs.',
			logo: {
				light: './src/assets/logo.svg',
				dark: './src/assets/logo-dark.svg',
				replacesTitle: true,
				alt: 'EdgeParse',
			},
			favicon: '/favicon.svg',
			lastUpdated: true,
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/raphaelmansuy/edgeparse' },
			],
			editLink: {
				baseUrl: 'https://github.com/raphaelmansuy/edgeparse/edit/main/site/',
			},
			customCss: [
				'./src/styles/tokens.css',
				'./src/styles/global.css',
			],
			components: {
				Hero: './src/components/landing/Hero.astro',
				SocialIcons: './src/components/SocialIcons.astro',
				Footer: './src/components/landing/Footer.astro',
			},
			head: [
				// Sitemap discovery link
				{
					tag: 'link',
					attrs: {
						rel: 'sitemap',
						href: '/sitemap-index.xml',
					},
				},
				// Preconnect to font services
				{
					tag: 'link',
					attrs: {
						rel: 'preconnect',
						href: 'https://fonts.googleapis.com',
					},
				},
				{
					tag: 'link',
					attrs: {
						rel: 'preconnect',
						href: 'https://fonts.gstatic.com',
						crossorigin: true,
					},
				},
				{
					tag: 'link',
					attrs: {
						rel: 'stylesheet',
						href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500&display=swap',
					},
				},
				// OpenGraph meta tags
				{
					tag: 'meta',
					attrs: {
						property: 'og:type',
						content: 'website',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:site_name',
						content: 'EdgeParse',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image',
						content: ogImageUrl,
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:width',
						content: '1200',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:height',
						content: '630',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:type',
						content: 'image/png',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:alt',
						content: 'EdgeParse – High-performance PDF-to-structured-data extraction engine. Rust-native, 10-100× faster.',
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:image:secure_url',
						content: ogImageUrl,
					},
				},
				{
					tag: 'meta',
					attrs: {
						property: 'og:locale',
						content: 'en_US',
					},
				},
				// Twitter Card meta tags
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:card',
						content: 'summary_large_image',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:site',
						content: '@rapaborges',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:creator',
						content: '@rapaborges',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:image',
						content: ogImageUrl,
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:image:alt',
						content: 'EdgeParse – High-performance PDF-to-structured-data extraction engine. Rust-native, 10-100× faster.',
					},
				},
				// Additional SEO meta tags
				{
					tag: 'meta',
					attrs: {
						name: 'author',
						content: 'Raphael Mansuy',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'keywords',
					content: 'PDF parser, PDF extraction, Rust PDF, structured data, RAG pipeline, table extraction, reading order, Python PDF, Node.js PDF, edgeparse, open source, AI agent, PDF to JSON, PDF to markdown, WebAssembly PDF, agent skill, LLM documents',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'robots',
						content: 'index, follow',
					},
				},
				// og:url — canonical URL for social sharing
				{
					tag: 'meta',
					attrs: {
						property: 'og:url',
						content: siteUrl,
					},
				},
				// NOTE: canonical is handled per-page by Starlight; do not set a global one.
				// JSON-LD: SoftwareApplication
				{
					tag: 'script',
					attrs: { type: 'application/ld+json' },
					content: JSON.stringify({
						'@context': 'https://schema.org',
						'@type': 'SoftwareApplication',
						name: 'EdgeParse',
						description: 'High-performance PDF-to-structured-data extraction engine, written in Rust. 10-100× faster than Python alternatives.',
						applicationCategory: 'DeveloperApplication',
						operatingSystem: 'macOS, Linux, Windows',
						offers: { '@type': 'Offer', price: '0', priceCurrency: 'USD' },
						author: { '@type': 'Person', name: 'Raphael Mansuy', url: 'https://github.com/raphaelmansuy' },
						url: fullUrl,
						downloadUrl: 'https://pypi.org/project/edgeparse/',
						softwareVersion: '0.2.1',
						license: 'https://opensource.org/licenses/Apache-2.0',
						programmingLanguage: ['Rust', 'Python', 'TypeScript'],
						image: ogImageUrl,
						screenshot: ogImageUrl,
						aggregateRating: {
							'@type': 'AggregateRating',
							ratingValue: '4.8',
							ratingCount: '50',
							bestRating: '5',
						},
					}),
				},
				// JSON-LD: SoftwareSourceCode (separate entity for codeRepository)
				{
					tag: 'script',
					attrs: { type: 'application/ld+json' },
					content: JSON.stringify({
						'@context': 'https://schema.org',
						'@type': 'SoftwareSourceCode',
						name: 'EdgeParse',
						codeRepository: 'https://github.com/raphaelmansuy/edgeparse',
						programmingLanguage: ['Rust', 'Python', 'TypeScript'],
						license: 'https://opensource.org/licenses/Apache-2.0',
						author: { '@type': 'Person', name: 'Raphael Mansuy', url: 'https://github.com/raphaelmansuy' },
					}),
				},
				// JSON-LD: Organization
				{
					tag: 'script',
					attrs: { type: 'application/ld+json' },
					content: JSON.stringify({
						'@context': 'https://schema.org',
						'@type': 'Organization',
						name: 'EdgeParse',
						url: fullUrl,
						logo: `${fullUrl}/favicon.svg`,
						sameAs: [
							'https://github.com/raphaelmansuy/edgeparse',
							'https://pypi.org/project/edgeparse/',
							'https://www.npmjs.com/package/edgeparse',
							'https://crates.io/crates/edgeparse-cli',
							'https://crates.io/crates/edgeparse-core',
						],
					}),
				},
				// JSON-LD: FAQPage for common questions
				{
					tag: 'script',
					attrs: { type: 'application/ld+json' },
					content: JSON.stringify({
						'@context': 'https://schema.org',
						'@type': 'FAQPage',
						mainEntity: [
							{
								'@type': 'Question',
								name: 'What is EdgeParse?',
								acceptedAnswer: {
									'@type': 'Answer',
									text: 'EdgeParse is a high-performance PDF-to-structured-data extraction engine written in Rust. It converts complex PDFs into clean, structured JSON, Markdown, or HTML in milliseconds without ML dependencies.',
								},
							},
							{
								'@type': 'Question',
								name: 'How fast is EdgeParse compared to other PDF parsers?',
								acceptedAnswer: {
									'@type': 'Answer',
									text: 'EdgeParse processes 40+ pages per second — 10 to 100× faster than Python-based alternatives like Docling or Marker. It achieves 0.026s average processing time per document.',
								},
							},
							{
								'@type': 'Question',
								name: 'What programming languages does EdgeParse support?',
								acceptedAnswer: {
									'@type': 'Answer',
									text: 'EdgeParse provides native bindings for Python (via PyO3), Node.js (via NAPI-RS), a standalone CLI binary, and can be used directly as a Rust library crate.',
								},
							},
							{
								'@type': 'Question',
								name: 'Does EdgeParse require GPU or ML models?',
								acceptedAnswer: {
									'@type': 'Answer',
									text: 'No. EdgeParse is a rule-based extraction engine with zero ML dependencies. No GPU, no Java, no Poppler, no Tesseract required. Just pip install edgeparse and go.',
								},
							},
						],
					}),
				},
				// JSON-LD: BreadcrumbList
				{
					tag: 'script',
					attrs: { type: 'application/ld+json' },
					content: JSON.stringify({
						'@context': 'https://schema.org',
						'@type': 'BreadcrumbList',
						itemListElement: [
							{ '@type': 'ListItem', position: 1, name: 'EdgeParse', item: fullUrl },
							{ '@type': 'ListItem', position: 2, name: 'Documentation', item: `${fullUrl}/getting-started/quick-start-python/` },
							{ '@type': 'ListItem', position: 3, name: 'Contact', item: `${fullUrl}/contact/` },
						],
					}),
				},
				// JSON-LD: ContactPage
				{
					tag: 'script',
					attrs: { type: 'application/ld+json' },
					content: JSON.stringify({
						'@context': 'https://schema.org',
						'@type': 'ContactPage',
						name: 'Contact EdgeParse',
						url: `${fullUrl}/contact/`,
						description: 'Contact the EdgeParse team via email, GitHub Discussions, or Elitizon for enterprise engagements.',
						contactType: 'customer support',
						email: 'contact@elitizon.com',
						areaServed: 'Worldwide',
						availableLanguage: 'English',
					}),
				},
			],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Python', slug: 'getting-started/quick-start-python' },
						{ label: 'Node.js', slug: 'getting-started/quick-start-nodejs' },
						{ label: 'CLI', slug: 'getting-started/quick-start-cli' },
						{ label: 'Rust', slug: 'getting-started/quick-start-rust' },
						{ label: 'WebAssembly', slug: 'getting-started/quick-start-wasm' },
					],
				},
				{
					label: 'Core Concepts',
					items: [
						{ label: 'Reading Order', slug: 'concepts/reading-order' },
						{ label: 'Table Extraction', slug: 'concepts/table-extraction' },
						{ label: 'Heading Detection', slug: 'concepts/heading-detection' },
						{ label: 'AI Safety Filters', slug: 'concepts/ai-safety' },
						{ label: 'Tagged PDF', slug: 'concepts/tagged-pdf' },
					],
				},
				{
					label: 'Output Formats',
					items: [
						{ label: 'JSON Schema', slug: 'output/json-schema' },
						{ label: 'Markdown', slug: 'output/markdown' },
						{ label: 'HTML', slug: 'output/html' },
						{ label: 'Plain Text', slug: 'output/plain-text' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'Batch Processing', slug: 'guides/batch-processing' },
						{ label: 'Docker', slug: 'guides/docker' },
						{ label: 'RAG Integration', slug: 'guides/rag-integration' },
						{ label: 'Hybrid Mode', slug: 'guides/hybrid-mode' },
						{ label: 'Image Extraction', slug: 'guides/image-extraction' },
						{ label: 'WASM Use Cases', slug: 'guides/wasm-use-cases' },
					],
				},
				{
					label: 'API Reference',
					items: [
						{ label: 'Python API', slug: 'api/python' },
						{ label: 'Node.js API', slug: 'api/nodejs' },
						{ label: 'CLI Reference', slug: 'api/cli' },
						{ label: 'Rust API', slug: 'api/rust' },
						{ label: 'ProcessingConfig', slug: 'api/processing-config' },
						{ label: 'WebAssembly API', slug: 'api/wasm' },
					],
				},
				{
					label: 'Benchmark',
					items: [
						{ label: 'Results', slug: 'benchmark/results' },
						{ label: 'Running Your Own', slug: 'benchmark/running' },
						{ label: 'Metrics Explained', slug: 'benchmark/metrics' },
					],
				},
				{
					label: 'Contributing',
					items: [
						{ label: 'Development Setup', slug: 'contributing/setup' },
						{ label: 'Architecture', slug: 'contributing/architecture' },
					],
				},
				{
					label: 'Releases',
					items: [
						{ label: 'Changelog', slug: 'changelog' },
					],
				},
				{
					label: 'More',
					items: [
						{ label: 'Enterprise', slug: 'enterprise' },
						{ label: 'Contact', slug: 'contact' },
					],
				},
			],
		}),
	],
	vite: {
		plugins: [tailwindcss()],
	},
});
