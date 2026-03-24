/** SplitPane — draggable vertical divider between left and right panes. */

import { el } from '../utils/dom';

export function createSplitPane(
  left: HTMLElement,
  right: HTMLElement,
): HTMLElement {
  const wrapper = el('div', { className: 'split-pane' });
  const handle = el('div', {
    className: 'split-pane__handle',
    role: 'separator',
    ariaValueMin: '20',
    ariaValueMax: '80',
    ariaValueNow: '50',
    tabIndex: '0',
  });

  left.classList.add('split-pane__left');
  right.classList.add('split-pane__right');

  wrapper.appendChild(left);
  wrapper.appendChild(handle);
  wrapper.appendChild(right);

  let dragging = false;

  function setRatio(pct: number) {
    const clamped = Math.max(20, Math.min(80, pct));
    left.style.width = `${clamped}%`;
    right.style.width = `${100 - clamped}%`;
    handle.setAttribute('aria-valuenow', String(Math.round(clamped)));
  }

  handle.addEventListener('mousedown', (e: Event) => {
    e.preventDefault();
    dragging = true;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  });

  document.addEventListener('mousemove', (e: MouseEvent) => {
    if (!dragging) return;
    const rect = wrapper.getBoundingClientRect();
    const pct = ((e.clientX - rect.left) / rect.width) * 100;
    setRatio(pct);
  });

  document.addEventListener('mouseup', () => {
    if (dragging) {
      dragging = false;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    }
  });

  // Keyboard support
  handle.addEventListener('keydown', (e: Event) => {
    const ke = e as KeyboardEvent;
    const current = parseFloat(handle.getAttribute('aria-valuenow') ?? '50');
    if (ke.key === 'ArrowLeft') setRatio(current - 2);
    else if (ke.key === 'ArrowRight') setRatio(current + 2);
  });

  setRatio(50);
  return wrapper;
}
