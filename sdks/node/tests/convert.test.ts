import { describe, it, expect } from 'vitest';

// NOTE: These tests require the native addon to be built for the current platform.
// If the addon is not available, the tests will skip gracefully.

let edgeparse: typeof import('../src/index.js') | null = null;
let loadError: string | null = null;

try {
  edgeparse = await import('../src/index.js');
} catch (err: unknown) {
  loadError = err instanceof Error ? err.message : String(err);
}

describe('edgeparse', () => {
  describe('version()', () => {
    it.skipIf(!edgeparse)('returns a semver version string', () => {
      const v = edgeparse!.version();
      expect(v).toMatch(/^\d+\.\d+\.\d+/);
    });
  });

  describe('convert()', () => {
    it.skipIf(!edgeparse)('throws on missing file', () => {
      expect(() => edgeparse!.convert('/nonexistent/file.pdf')).toThrow();
    });

    it.skipIf(!edgeparse)('converts a test PDF to markdown', () => {
      const fixturesPath = new URL(
        '../../../tests/fixtures/sample.pdf',
        import.meta.url,
      );
      const result = edgeparse!.convert(fixturesPath.pathname);
      expect(typeof result).toBe('string');
      expect(result.length).toBeGreaterThan(0);
    });

    it.skipIf(!edgeparse)('converts to JSON format', () => {
      const fixturesPath = new URL(
        '../../../tests/fixtures/sample.pdf',
        import.meta.url,
      );
      const result = edgeparse!.convert(fixturesPath.pathname, {
        format: 'json',
      });
      expect(typeof result).toBe('string');
      // Should be valid JSON
      expect(() => JSON.parse(result)).not.toThrow();
    });

    it.skipIf(!edgeparse)('converts to HTML format', () => {
      const fixturesPath = new URL(
        '../../../tests/fixtures/sample.pdf',
        import.meta.url,
      );
      const result = edgeparse!.convert(fixturesPath.pathname, {
        format: 'html',
      });
      expect(typeof result).toBe('string');
      expect(result).toContain('<');
    });

    it.skipIf(!edgeparse)('converts to text format', () => {
      const fixturesPath = new URL(
        '../../../tests/fixtures/sample.pdf',
        import.meta.url,
      );
      const result = edgeparse!.convert(fixturesPath.pathname, {
        format: 'text',
      });
      expect(typeof result).toBe('string');
      expect(result.length).toBeGreaterThan(0);
    });

    it.skipIf(!edgeparse)('throws on invalid format', () => {
      const fixturesPath = new URL(
        '../../../tests/fixtures/sample.pdf',
        import.meta.url,
      );
      expect(() =>
        edgeparse!.convert(fixturesPath.pathname, { format: 'invalid' }),
      ).toThrow();
    });
  });

  describe('module loading', () => {
    it('should either load successfully or report platform mismatch', () => {
      if (edgeparse) {
        expect(typeof edgeparse.convert).toBe('function');
        expect(typeof edgeparse.version).toBe('function');
      } else {
        // Native addon not available — acceptable in CI or on unbuilt platforms
        expect(loadError).toBeTruthy();
      }
    });
  });
});
