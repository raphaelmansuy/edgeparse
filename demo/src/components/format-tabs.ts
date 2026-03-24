/** FormatTabs — JSON | Markdown | HTML | Text tab switcher.
 *
 * When a format is being rendered (async Markdown/HTML conversion), the active
 * tab shows an animated pulse dot via the `--loading` modifier class. This
 * gives the user immediate feedback that the tab click was registered and work
 * is happening — without blanking the content pane.
 */

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

  // Sync active-tab styling whenever outputFormat changes.
  store.subscribe('outputFormat', (value) => {
    const current = value as OutputFormat;
    for (const btn of buttons) {
      const active = btn.dataset.format === current;
      btn.classList.toggle('format-tabs__tab--active', active);
      btn.setAttribute('aria-selected', String(active));
    }
  });

  // Show a pulse dot on the active tab while an async render is in progress.
  // This fires when the OutputViewer sets store.renderStatus = 'rendering'.
  store.subscribe('renderStatus', (value) => {
    const isRendering = value === 'rendering';
    const currentFormat = store.get('outputFormat');
    for (const btn of buttons) {
      const isActive = btn.dataset.format === currentFormat;
      btn.classList.toggle('format-tabs__tab--loading', isActive && isRendering);
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
