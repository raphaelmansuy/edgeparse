/** Accessible model-download consent dialog (replaces window.confirm). */

export interface ConsentRequest {
  id: string;
  mb: string;
}

/**
 * Show a modal asking the user to download an OCR model.
 * Resolves true if accepted, false if declined / dismissed.
 */
export function showModelConsent(req: ConsentRequest): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'consent-overlay';
    overlay.setAttribute('role', 'presentation');

    const dialog = document.createElement('div');
    dialog.className = 'consent-dialog';
    dialog.setAttribute('role', 'alertdialog');
    dialog.setAttribute('aria-modal', 'true');
    dialog.setAttribute('aria-labelledby', 'consent-title');
    dialog.setAttribute('aria-describedby', 'consent-desc');

    dialog.innerHTML = `
      <h2 id="consent-title">Download OCR model?</h2>
      <p id="consent-desc">
        Model <strong>${escapeHtml(req.id)}</strong> (~${escapeHtml(req.mb)}&nbsp;MB)
        is needed only for image-embedded tables. Decline to continue with PDF text only
        (quality: degraded for scanned tables).
      </p>
      <div class="consent-dialog__actions">
        <button type="button" class="consent-dialog__decline" data-action="decline">Decline</button>
        <button type="button" class="consent-dialog__accept" data-action="accept" autofocus>Download</button>
      </div>
    `;

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    const previouslyFocused = document.activeElement as HTMLElement | null;
    const acceptBtn = dialog.querySelector('[data-action="accept"]') as HTMLButtonElement;
    acceptBtn.focus();

    const cleanup = (accepted: boolean) => {
      document.removeEventListener('keydown', onKey);
      overlay.remove();
      previouslyFocused?.focus?.();
      resolve(accepted);
    };

    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        cleanup(false);
      }
      if (e.key === 'Tab') {
        const focusable = dialog.querySelectorAll<HTMLElement>('button');
        if (focusable.length < 2) return;
        const first = focusable[0]!;
        const last = focusable[focusable.length - 1]!;
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener('keydown', onKey);

    dialog.addEventListener('click', (e) => {
      const t = (e.target as HTMLElement).closest('[data-action]') as HTMLElement | null;
      if (!t) return;
      cleanup(t.dataset.action === 'accept');
    });

    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) cleanup(false);
    });
  });
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}
