/** Legend — semantic type filter chip toggles. */

import { el } from '../utils/dom';
import { store } from '../state';
import type { SemanticType } from '../types';
import { SEMANTIC_BORDER_COLORS } from '../utils/colors';

const ALL_TYPES: SemanticType[] = [
  'TextBlock', 'Heading', 'Table', 'TableBorder',
  'Figure', 'Image', 'Line', 'LineArt', 'List', 'Other',
];

export function createLegend(): HTMLElement {
  const panel = el('div', {
    className: 'legend',
    role: 'group',
    ariaLabel: 'Toggle overlay element types',
  });
  const title = el('span', { className: 'legend__title', ariaHidden: 'true' });
  title.textContent = 'Types';
  panel.appendChild(title);

  for (const type of ALL_TYPES) {
    const active = store.get('activeSemanticFilters').has(type);
    const chip = el('button', {
      className: `legend__chip${active ? ' legend__chip--active' : ''}`,
      ariaPressed: String(active),
    }) as HTMLButtonElement;
    chip.title = `Toggle ${type} bounding boxes`;
    chip.textContent = type;
    chip.style.setProperty('--chip-color', SEMANTIC_BORDER_COLORS[type]);

    chip.addEventListener('click', () => {
      const current = new Set(store.get('activeSemanticFilters'));
      const isOn = current.has(type);
      if (isOn) current.delete(type); else current.add(type);
      chip.classList.toggle('legend__chip--active', !isOn);
      chip.setAttribute('aria-pressed', String(!isOn));
      store.set('activeSemanticFilters', current);
    });

    panel.appendChild(chip);
  }

  return panel;
}
