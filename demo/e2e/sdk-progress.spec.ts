/**
 * SDK model/job observability e2e (extends the demo smoke suite).
 * Full OCR download flows need network; here we assert progress wiring exists.
 */
import { test, expect } from '@playwright/test';

test.describe('EdgeParse Web SDK integration', () => {
  test('progress bar mounts and exposes model card hooks', async ({ page }) => {
    await page.goto('/');
    const bar = page.locator('.progress-bar');
    await expect(bar).toBeAttached();
    await expect(page.locator('.progress-bar__model-card')).toBeAttached();
    await expect(page.locator('.progress-bar__offline')).toBeAttached();
  });

  test('offline indicator can be toggled via snapshot event', async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => {
      window.dispatchEvent(
        new CustomEvent('edgeparse:snapshot', {
          detail: {
            engine: { state: 'ready' },
            models: {},
            jobs: {},
            capabilities: null,
            online: false,
          },
        }),
      );
    });
    await expect(page.locator('.progress-bar__offline')).toBeVisible();
  });

  test('job progress updates the label', async ({ page }) => {
    await page.goto('/');
    await page.evaluate(() => {
      window.dispatchEvent(
        new CustomEvent('edgeparse:job-progress', {
          detail: {
            jobId: 't1',
            state: 'ocr',
            fraction: 0.42,
            label: 'OCR 1/2',
            ocrDone: 1,
            ocrTotal: 2,
            warnings: [],
          },
        }),
      );
    });
    await expect(page.locator('.progress-bar__label')).toContainText('OCR 1/2');
  });
});
