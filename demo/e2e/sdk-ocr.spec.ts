/**
 * OCR consent / quality / progress e2e.
 */
import { test, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

test.describe('OCR consent and quality badge', () => {
  test('consent dialog decline yields degraded quality event wiring', async ({
    page,
  }) => {
    await page.goto('/');
    // Simulate consent decline path via parse-result event (no large model download in CI)
    await page.evaluate(() => {
      window.dispatchEvent(
        new CustomEvent('edgeparse:parse-result', {
          detail: {
            quality: 'degraded',
            warnings: ['OCR models policy declined'],
          },
        }),
      );
    });
    const badge = page.locator('[data-testid="quality-badge"]');
    await expect(badge).toBeVisible();
    await expect(badge).toContainText('degraded');
  });

  test('consent dialog is accessible when opened', async ({ page }) => {
    await page.goto('/');
    await page.evaluate(async () => {
      const mod = await import('/src/components/consent-dialog.ts');
      // fire-and-forget — we assert DOM then dismiss
      void mod.showModelConsent({ id: 'ppocrv6-det-small', mb: '9.5' });
    });
    const dialog = page.locator('[role="alertdialog"]');
    await expect(dialog).toBeVisible();
    await expect(dialog.locator('#consent-title')).toContainText('Download OCR');
    await page.locator('.consent-dialog__decline').click();
    await expect(dialog).toHaveCount(0);
  });

  test('model download progress card updates from snapshot', async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => {
      window.dispatchEvent(
        new CustomEvent('edgeparse:snapshot', {
          detail: {
            engine: { state: 'ready' },
            models: {
              'ppocrv6-det-small': {
                id: 'ppocrv6-det-small',
                state: 'downloading',
                loaded: 1024 * 1024,
                total: 10 * 1024 * 1024,
                bytesPerSec: 1024 * 1024,
                etaSec: 9,
                source: 'network',
              },
            },
            jobs: {},
            capabilities: null,
            online: true,
          },
        }),
      );
    });
    await expect(page.locator('.progress-bar__model-card')).toBeVisible();
    await expect(page.locator('.progress-bar__model-title')).toContainText(
      'ppocrv6-det-small',
    );
    await expect(page.locator('.progress-bar__model-meta')).toContainText('MB');
  });
});
