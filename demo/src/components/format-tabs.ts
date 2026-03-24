/** FormatTabs — JSON | Markdown | HTML | Text tab switcher. */

import { el } from '../utils/dom';
import { store } from '../state';
import type { OutputFormat } from '../types';
import { switchFormat } from '../services/format-converter';

const FORMATS: OutputFormat[] = ['json', 'markdown', 'html', 'text'];

export function createFormatTabs(): HTMLElement {
  const nav = el('nav', {
    className: 'format-tabs',
    role: 'tablist',
    ariaLabel: 'Output format',
  });

  const buttons: HTMLButtonElement[] = FORMATS.map((fmt) => {
    const btn = el('button', {
      className: 'format-tabs__tab',
      role: 'tab',
      textContent: fmt.toUpperCase(),
      ariaSelected: fmt === store.get('outputFormat') ? 'true' : 'false',
    }) as HTMLButtonElement;

    btn.dataset.format = fmt;
    btn.addEventListener('click', () => switchFormat(fmt));
    nav.appendChild(btn);
    return btn;
  });

  store.subscribe('outputFormat', (value) => {
    const current = value as OutputFormat;
    for (const btn of buttons) {
      const active = btn.dataset.format === current;
      btn.classList.toggle('format-tabs__tab--active', active);
      btn.setAttribute('aria-selected', String(active));
    }
  });

  // Set initial state
  const initial = store.get('outputFormat');
  for (const btn of buttons) {
    const active = btn.dataset.format === initial;
    btn.classList.toggle('format-tabs__tab--active', active);
  }

  return nav;
}
