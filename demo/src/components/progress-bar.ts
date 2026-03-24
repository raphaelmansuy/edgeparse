/** ProgressBar — WASM loading + parsing progress indicator. */

import { el } from '../utils/dom';
import { store } from '../state';

export function createProgressBar(): HTMLElement {
  const bar = el('div', {
    className: 'progress-bar',
    innerHTML: `<progress class="progress-bar__indicator" max="100" value="0"></progress>
    <span class="progress-bar__label"></span>`,
  });

  const indicator = bar.querySelector('progress') as HTMLProgressElement;
  const label = bar.querySelector('.progress-bar__label') as HTMLSpanElement;

  function update() {
    const wasmStatus = store.get('wasmStatus');
    const parseStatus = store.get('parseStatus');

    if (wasmStatus === 'loading') {
      bar.classList.add('progress-bar--active');
      indicator.removeAttribute('value'); // indeterminate
      label.textContent = 'Loading parser…';
    } else if (parseStatus === 'parsing') {
      bar.classList.add('progress-bar--active');
      indicator.removeAttribute('value');
      label.textContent = 'Parsing PDF…';
    } else {
      bar.classList.remove('progress-bar--active');
      indicator.value = parseStatus === 'done' ? 100 : 0;
      label.textContent = '';
    }
  }

  store.subscribe('wasmStatus', update);
  store.subscribe('parseStatus', update);
  update();

  return bar;
}
