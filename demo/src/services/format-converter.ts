/** Format converter — switches output format using cached results. */

import { store } from '../state';
import type { OutputFormat } from '../types';

export function switchFormat(format: OutputFormat): void {
  store.set('outputFormat', format);

  const cache = store.get('formatCache');
  if (!cache) return;

  const output = cache[format];
  if (output != null) {
    store.set('outputText', output);
  }
}
