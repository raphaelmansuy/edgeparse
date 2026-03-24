/** PageNav — page navigation controls. */

import { el } from '../utils/dom';
import { store } from '../state';

export function createPageNav(): HTMLElement {
  const nav = el('div', {
    className: 'page-nav',
    innerHTML: `
      <button class="page-nav__btn" data-action="prev" aria-label="Previous page">&larr;</button>
      <span class="page-nav__info">
        <input class="page-nav__input" type="number" min="1" value="1" aria-label="Page number" />
        <span class="page-nav__sep"> / </span>
        <span class="page-nav__total">0</span>
      </span>
      <button class="page-nav__btn" data-action="next" aria-label="Next page">&rarr;</button>
    `,
  });

  const input = nav.querySelector('.page-nav__input') as HTMLInputElement;
  const total = nav.querySelector('.page-nav__total') as HTMLSpanElement;
  const prevBtn = nav.querySelector('[data-action="prev"]') as HTMLButtonElement;
  const nextBtn = nav.querySelector('[data-action="next"]') as HTMLButtonElement;

  function updateButtons() {
    const cur = store.get('currentPage');
    const max = store.get('pageCount');
    prevBtn.disabled = cur <= 1;
    nextBtn.disabled = cur >= max;
    input.value = String(cur);
    input.max = String(max);
    total.textContent = String(max);
  }

  prevBtn.addEventListener('click', () => {
    const cur = store.get('currentPage');
    if (cur > 1) store.set('currentPage', cur - 1);
  });

  nextBtn.addEventListener('click', () => {
    const cur = store.get('currentPage');
    if (cur < store.get('pageCount')) store.set('currentPage', cur + 1);
  });

  input.addEventListener('change', () => {
    const val = parseInt(input.value, 10);
    const max = store.get('pageCount');
    if (val >= 1 && val <= max) {
      store.set('currentPage', val);
    } else {
      input.value = String(store.get('currentPage'));
    }
  });

  store.subscribe('currentPage', updateButtons);
  store.subscribe('pageCount', updateButtons);
  updateButtons();

  return nav;
}
