/**
 * EdgeParse Demo — End-to-End Tests
 *
 * Tests run against the locally-running Vite dev server (http://localhost:5173).
 * Start the server with `npm run dev` before running these tests.
 *
 * Usage:
 *   npm run test:e2e          # run all tests
 *   npm run test:e2e:headed   # with a real browser window
 */

import { test, expect, type Page } from '@playwright/test';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ── Fixtures ──────────────────────────────────────────────────────────────────

const SAMPLE_PDF = path.resolve(__dirname, '../public/sample.pdf');
const LOREM_PDF  = path.resolve(__dirname, '../../examples/pdf/lorem.pdf');

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Wait until parsing is complete: progress bar is no longer active AND
 * the CodeMirror editor contains JSON output.
 */
async function waitForParsedOutput(page: Page): Promise<void> {
  // Wait for progress bar to stop showing
  await page.waitForFunction(
    () => !document.querySelector('.progress-bar--active'),
    { timeout: 30_000 },
  );

  // Wait for CodfeMirror to show JSON
  await page.waitForFunction(
    () => {
      const cm = document.querySelector('.cm-content');
      return cm?.textContent?.trimStart().startsWith('{');
    },
    { timeout: 30_000 },
  );
}

/**
 * Click "Upload PDF", choose a file, and wait for CodeMirror to show
 * different content (indicating the new PDF was compiled).
 * Robust against parse being so fast the progress-bar never enters "active".
 */
async function uploadPdf(page: Page, pdfPath: string): Promise<void> {
  // Snapshot current content so we can detect the change
  const prevContent = await page.locator('.cm-content').textContent() ?? '';

  const [fileChooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.locator('.toolbar__btn--upload').click(),
  ]);
  await fileChooser.setFiles(pdfPath);

  // Wait until CodeMirror is updated with new content from the uploaded PDF
  await page.waitForFunction(
    (prev: string) => {
      const cm = document.querySelector('.cm-content');
      const cur = cm?.textContent ?? '';
      return cur !== prev && cur.trimStart().startsWith('{');
    },
    prevContent,
    { timeout: 45_000 },
  );
}

// ── Spec ──────────────────────────────────────────────────────────────────────

test.describe('EdgeParse Demo', () => {

  // ── 1. Initial page load ────────────────────────────────────────────────────

  test('1.1 − page title is present', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle(/EdgeParse/i);
  });

  test('1.2 − key toolbar elements are visible', async ({ page }) => {
    await page.goto('/');

    // Upload button (text content "Upload PDF")
    await expect(page.locator('.toolbar__btn--upload')).toBeVisible();
    await expect(page.locator('.toolbar__btn--upload')).toHaveText('Upload PDF');

    // Format tabs
    for (const fmt of ['JSON', 'MARKDOWN', 'HTML', 'TEXT']) {
      await expect(page.getByRole('tab', { name: fmt })).toBeVisible();
    }

    // Overlay & dark-mode controls
    await expect(page.locator('.toolbar__btn--overlay')).toBeVisible();
    await expect(page.locator('.toolbar__btn--dark')).toBeVisible();

    // Output actions
    await expect(page.getByRole('button', { name: /copy/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /download/i })).toBeVisible();
  });

  test('1.3 − default PDF is loaded and parsed automatically', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    const content = await page.locator('.cm-content').textContent();
    expect(content).toContain('"file_name"');
    expect(content).toContain('"number_of_pages"');

    // Page nav shows 29 pages (leworld PDF)
    await expect(page.locator('.page-nav__total')).toHaveText('29');
  });

  test('1.4 − PDF canvas renders with non-zero dimensions', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    const canvas = page.locator('.pdf-viewer__canvas');
    await expect(canvas).toBeVisible();
    const box = await canvas.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeGreaterThan(100);
    expect(box!.height).toBeGreaterThan(100);
  });

  test('1.5 − no error toast on initial load', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);
    await expect(page.locator('.toast--error')).not.toBeVisible();
  });

  test('1.6 − no critical JavaScript errors on load', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (err) => errors.push(err.message));
    await page.goto('/');
    await waitForParsedOutput(page);
    const critical = errors.filter(
      (e) => !e.includes('Rendering cancelled') && !e.includes('worker'),
    );
    expect(critical).toHaveLength(0);
  });

  // ── 2. File upload ──────────────────────────────────────────────────────────

  test('2.1 − clicking "Upload PDF" opens a file chooser', async ({ page }) => {
    await page.goto('/');

    const fileChooserPromise = page.waitForEvent('filechooser');
    await page.locator('.toolbar__btn--upload').click();
    const fc = await fileChooserPromise;
    expect(fc.isMultiple()).toBe(false);
  });

  test('2.2 − uploading lorem.pdf updates content without any error toast', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await uploadPdf(page, LOREM_PDF);

    // Lorem PDF has 1 page
    await page.waitForFunction(
      () => document.querySelector('.cm-content')?.textContent?.includes('"number_of_pages": 1'),
      { timeout: 30_000 },
    );

    // No error toast
    await expect(page.locator('.toast--error')).not.toBeVisible();

    // Page nav updated
    await expect(page.locator('.page-nav__total')).toHaveText('1');
  });

  test('2.3 − uploading sample.pdf works without "No PDF loaded" error', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await uploadPdf(page, SAMPLE_PDF);

    await page.waitForFunction(
      () => document.querySelector('.cm-content')?.textContent?.trimStart().startsWith('{'),
      { timeout: 30_000 },
    );

    await expect(page.locator('.toast--error')).not.toBeVisible();

    const canvas = page.locator('.pdf-viewer__canvas');
    await expect(canvas).toBeVisible();
    const box = await canvas.boundingBox();
    expect(box!.width).toBeGreaterThan(50);
  });

  test('2.4 − uploading a PDF resets page counter to 1', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    // Navigate to page 2 on leworld PDF
    await page.getByRole('button', { name: /next page/i }).click();
    await expect(
      page.getByRole('spinbutton', { name: /page number/i }),
    ).toHaveValue('2');

    // Upload a new PDF
    await uploadPdf(page, LOREM_PDF);

    // Page counter should reset to 1
    await expect(
      page.getByRole('spinbutton', { name: /page number/i }),
    ).toHaveValue('1');
  });

  // ── 3. Format tabs ──────────────────────────────────────────────────────────

  test('3.1 − JSON tab (default) shows code editor with JSON content', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await expect(page.getByRole('tab', { name: 'JSON' })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.output-viewer__code')).toBeVisible();

    const text = await page.locator('.cm-content').textContent();
    expect(text).toContain('"file_name"');
  });

  test('3.2 − MARKDOWN tab shows rendered content', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('tab', { name: 'MARKDOWN' }).click();
    await expect(page.getByRole('tab', { name: 'MARKDOWN' })).toHaveAttribute('aria-selected', 'true');

    await expect(page.locator('.output-viewer__rendered')).toBeVisible();
    const content = await page.locator('.output-viewer__rendered').textContent();
    expect(content!.length).toBeGreaterThan(10);
  });

  test('3.3 − HTML tab shows rendered HTML content', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('tab', { name: 'HTML' }).click();
    await expect(page.getByRole('tab', { name: 'HTML' })).toHaveAttribute('aria-selected', 'true');

    await expect(page.locator('.output-viewer__rendered')).toBeVisible();
  });

  test('3.4 − TEXT tab shows plain text in code editor', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('tab', { name: 'TEXT' }).click();
    await expect(page.getByRole('tab', { name: 'TEXT' })).toHaveAttribute('aria-selected', 'true');

    await expect(page.locator('.output-viewer__code')).toBeVisible();
    await expect(page.locator('.output-viewer__rendered')).not.toBeVisible();
    const text = await page.locator('.cm-content').textContent();
    expect(text!.length).toBeGreaterThan(5);
  });

  test('3.5 − switching back to JSON shows JSON and hides rendered pane', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('tab', { name: 'MARKDOWN' }).click();
    await page.getByRole('tab', { name: 'JSON' }).click();

    await expect(page.locator('.output-viewer__code')).toBeVisible();
    await expect(page.locator('.output-viewer__rendered')).not.toBeVisible();
    const text = await page.locator('.cm-content').textContent();
    expect(text).toContain('"file_name"');
  });

  // ── 4. Page navigation ──────────────────────────────────────────────────────

  test('4.1 − previous button is disabled on page 1', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);
    await expect(page.getByRole('button', { name: /previous page/i })).toBeDisabled();
  });

  test('4.2 − next button advances the page', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('button', { name: /next page/i }).click();

    await expect(
      page.getByRole('spinbutton', { name: /page number/i }),
    ).toHaveValue('2');
    await expect(page.getByRole('button', { name: /previous page/i })).toBeEnabled();
  });

  test('4.3 − previous button goes back to page 1', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('button', { name: /next page/i }).click();
    await page.getByRole('button', { name: /previous page/i }).click();

    await expect(
      page.getByRole('spinbutton', { name: /page number/i }),
    ).toHaveValue('1');
    await expect(page.getByRole('button', { name: /previous page/i })).toBeDisabled();
  });

  test('4.4 − last page disables next button', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    // Upload lorem PDF (1 page) — next should be disabled
    await uploadPdf(page, LOREM_PDF);

    await page.waitForFunction(
      () => document.querySelector('.page-nav__total')?.textContent === '1',
      { timeout: 10_000 },
    );

    await expect(page.getByRole('button', { name: /next page/i })).toBeDisabled();
    await expect(page.getByRole('button', { name: /previous page/i })).toBeDisabled();
  });

  // ── 5. Overlay toggle ───────────────────────────────────────────────────────

  test('5.1 − overlay toggle starts in active state', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.toolbar__btn--overlay')).toHaveAttribute('aria-pressed', 'true');
  });

  test('5.2 − overlay toggle changes aria-pressed on click', async ({ page }) => {
    await page.goto('/');

    await page.locator('.toolbar__btn--overlay').click();
    await expect(page.locator('.toolbar__btn--overlay')).toHaveAttribute('aria-pressed', 'false');

    await page.locator('.toolbar__btn--overlay').click();
    await expect(page.locator('.toolbar__btn--overlay')).toHaveAttribute('aria-pressed', 'true');
  });

  // ── 6. Dark mode ────────────────────────────────────────────────────────────

  test('6.1 − dark mode toggle adds .dark to <html>', async ({ page }) => {
    await page.goto('/');

    await expect(page.locator('html')).not.toHaveClass(/dark/);

    await page.locator('.toolbar__btn--dark').click();
    await expect(page.locator('html')).toHaveClass(/dark/);

    await page.locator('.toolbar__btn--dark').click();
    await expect(page.locator('html')).not.toHaveClass(/dark/);
  });

  // ── 7. Drag-and-drop visual feedback ───────────────────────────────────────

  test('7.1 − dragover adds .drag-over class to body', async ({ page }) => {
    await page.goto('/');

    await page.evaluate(() => {
      document.body.dispatchEvent(
        new DragEvent('dragover', { bubbles: true, cancelable: true }),
      );
    });
    await expect(page.locator('body')).toHaveClass(/drag-over/);
  });

  test('7.2 − dragleave removes .drag-over class', async ({ page }) => {
    await page.goto('/');

    await page.evaluate(() => {
      document.body.dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true }));
    });
    await page.evaluate(() => {
      document.body.dispatchEvent(new DragEvent('dragleave', { bubbles: true }));
    });
    await expect(page.locator('body')).not.toHaveClass(/drag-over/);
  });

  // ── 8. Output actions ───────────────────────────────────────────────────────

  test('8.1 − copy button writes JSON to clipboard', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto('/');
    await waitForParsedOutput(page);

    await page.getByRole('button', { name: /copy/i }).click();
    await page.waitForTimeout(500);

    const clipText = await page.evaluate(() => navigator.clipboard.readText());
    expect(clipText).toContain('"file_name"');
  });

  // ── 9. Upload + all-formats round-trip ─────────────────────────────────────

  test('9.1 − after upload, all format tabs produce non-empty output', async ({ page }) => {
    await page.goto('/');
    await waitForParsedOutput(page);

    await uploadPdf(page, LOREM_PDF);

    await page.waitForFunction(
      () => document.querySelector('.cm-content')?.textContent?.includes('"number_of_pages": 1'),
      { timeout: 30_000 },
    );

    // JSON
    await page.getByRole('tab', { name: 'JSON' }).click();
    expect(await page.locator('.cm-content').textContent()).toContain('"file_name"');

    // MARKDOWN
    await page.getByRole('tab', { name: 'MARKDOWN' }).click();
    await expect(page.locator('.output-viewer__rendered')).toBeVisible();
    expect((await page.locator('.output-viewer__rendered').textContent())!.length).toBeGreaterThan(5);

    // HTML
    await page.getByRole('tab', { name: 'HTML' }).click();
    await expect(page.locator('.output-viewer__rendered')).toBeVisible();

    // TEXT
    await page.getByRole('tab', { name: 'TEXT' }).click();
    await expect(page.locator('.output-viewer__code')).toBeVisible();
    expect((await page.locator('.cm-content').textContent())!.length).toBeGreaterThan(5);

    // No errors throughout
    await expect(page.locator('.toast--error')).not.toBeVisible();
  });
});

