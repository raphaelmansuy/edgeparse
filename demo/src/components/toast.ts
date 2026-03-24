/** Toast — non-blocking notification component. */

import { el } from '../utils/dom';
import { store } from '../state';

const DISMISS_MS = 10_000;

let container: HTMLDivElement | null = null;

function getContainer(): HTMLDivElement {
  if (!container) {
    container = el('div', { className: 'toast-container' }) as HTMLDivElement;
    document.body.appendChild(container);
  }
  return container;
}

export function showToast(message: string, level: 'error' | 'info' = 'error'): void {
  const toast = el('div', {
    className: `toast toast--${level}`,
    role: 'alert',
    innerHTML: `<span class="toast__msg">${escapeHtml(message)}</span><button class="toast__close" aria-label="Dismiss">&times;</button>`,
  });

  const closeBtn = toast.querySelector('.toast__close')!;
  const dismiss = () => toast.remove();
  closeBtn.addEventListener('click', dismiss);
  const timer = setTimeout(dismiss, DISMISS_MS);
  toast.addEventListener('mouseenter', () => clearTimeout(timer));

  getContainer().appendChild(toast);
}

function escapeHtml(s: string): string {
  const d = document.createElement('div');
  d.textContent = s;
  return d.innerHTML;
}

/** Subscribe to error state changes and auto-show toasts. */
export function initToastListener(): void {
  store.subscribe('errorMessage', (value) => {
    const msg = value as string | null;
    if (msg) showToast(msg, 'error');
  });
}
