/** ProgressBar — job progress + model download card + quality / offline badges. */

import { el } from '../utils/dom';
import { store } from '../state';
import type { JobProgress, Snapshot } from '@edgeparse/web';

export function createProgressBar(): HTMLElement {
  const bar = el('div', {
    className: 'progress-bar',
    innerHTML: `
      <div class="progress-bar__row">
        <progress class="progress-bar__indicator" max="100" value="0" aria-label="Parse progress"></progress>
        <span class="progress-bar__label" role="status" aria-live="polite" aria-atomic="true"></span>
        <span class="progress-bar__quality" hidden data-testid="quality-badge"></span>
        <span class="progress-bar__offline" hidden role="status">Offline</span>
      </div>
      <div class="progress-bar__model-card" hidden>
        <div class="progress-bar__model-title"></div>
        <progress class="progress-bar__model-indicator" max="100" value="0" aria-label="Model download"></progress>
        <div class="progress-bar__model-meta" role="status" aria-live="polite"></div>
        <button type="button" class="progress-bar__model-retry" hidden>Retry download</button>
      </div>
    `,
  });

  const indicator = bar.querySelector('.progress-bar__indicator') as HTMLProgressElement;
  const label = bar.querySelector('.progress-bar__label') as HTMLSpanElement;
  const offline = bar.querySelector('.progress-bar__offline') as HTMLSpanElement;
  const quality = bar.querySelector('.progress-bar__quality') as HTMLSpanElement;
  const modelCard = bar.querySelector('.progress-bar__model-card') as HTMLDivElement;
  const modelTitle = bar.querySelector('.progress-bar__model-title') as HTMLDivElement;
  const modelIndicator = bar.querySelector(
    '.progress-bar__model-indicator',
  ) as HTMLProgressElement;
  const modelMeta = bar.querySelector('.progress-bar__model-meta') as HTMLDivElement;
  const retryBtn = bar.querySelector('.progress-bar__model-retry') as HTMLButtonElement;

  let lastFailedModel: string | null = null;

  function setJobProgress(p: JobProgress | null) {
    if (!p || p.state === 'done' || p.state === 'failed' || p.state === 'aborted') {
      const wasmStatus = store.get('wasmStatus');
      const parseStatus = store.get('parseStatus');
      if (wasmStatus === 'loading') {
        bar.classList.add('progress-bar--active');
        indicator.removeAttribute('value');
        label.textContent = 'Loading parser…';
      } else if (parseStatus === 'parsing') {
        bar.classList.add('progress-bar--active');
        indicator.removeAttribute('value');
        label.textContent = 'Parsing PDF…';
      } else if (p?.state === 'done') {
        bar.classList.remove('progress-bar--active');
        indicator.value = 100;
        label.textContent = p.label || 'Done';
      } else if (p == null && label.textContent && !label.textContent.endsWith('…')) {
        // Keep last explicit job label (e.g. from e2e / late events); do not wipe.
        return;
      } else {
        bar.classList.remove('progress-bar--active');
        indicator.value = parseStatus === 'done' ? 100 : 0;
        label.textContent = '';
      }
      return;
    }
    bar.classList.add('progress-bar--active');
    indicator.value = Math.round(p.fraction * 100);
    label.textContent = p.label || p.state;
  }

  function updateFromSnapshot(snap: Snapshot) {
    offline.hidden = snap.online;
    const downloading = Object.values(snap.models).find(
      (m) => m.state === 'downloading' || m.state === 'verifying' || m.state === 'failed',
    );
    if (!downloading) {
      modelCard.hidden = true;
      return;
    }
    modelCard.hidden = false;
    const pct =
      downloading.total > 0
        ? Math.round((downloading.loaded / downloading.total) * 100)
        : 0;
    modelTitle.textContent =
      downloading.state === 'failed'
        ? `Model ${downloading.id} failed`
        : `Downloading PP-OCR model ${downloading.id}`;
    if (downloading.state === 'downloading' && downloading.total <= 0) {
      modelIndicator.removeAttribute('value');
    } else {
      modelIndicator.value = pct;
    }
    const mb = (n: number) => (n / (1024 * 1024)).toFixed(1);
    const eta =
      downloading.etaSec != null ? ` · ETA ${Math.ceil(downloading.etaSec)}s` : '';
    const speed =
      downloading.bytesPerSec > 0
        ? ` · ${(downloading.bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
        : '';
    modelMeta.textContent = `${mb(downloading.loaded)} / ${mb(downloading.total)} MB${speed}${eta}`;
    lastFailedModel = downloading.state === 'failed' ? downloading.id : null;
    retryBtn.hidden = downloading.state !== 'failed';
  }

  retryBtn.addEventListener('click', () => {
    if (lastFailedModel) {
      window.dispatchEvent(
        new CustomEvent('edgeparse:model-retry', { detail: { id: lastFailedModel } }),
      );
    }
  });

  window.addEventListener('edgeparse:job-progress', ((e: CustomEvent<JobProgress>) => {
    setJobProgress(e.detail);
  }) as EventListener);

  window.addEventListener('edgeparse:snapshot', ((e: CustomEvent<Snapshot>) => {
    updateFromSnapshot(e.detail);
  }) as EventListener);

  window.addEventListener(
    'edgeparse:parse-result',
    ((e: CustomEvent<{ quality: string; warnings: string[] }>) => {
      const q = e.detail.quality;
      quality.hidden = false;
      quality.dataset.quality = q;
      quality.textContent = q === 'full' ? 'Quality: full' : 'Quality: degraded';
      quality.title = e.detail.warnings?.join('\n') || '';
    }) as EventListener,
  );

  store.subscribe('wasmStatus', () => setJobProgress(null));
  store.subscribe('parseStatus', () => setJobProgress(null));
  setJobProgress(null);

  return bar;
}
