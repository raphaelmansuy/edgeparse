/** AppShell — top-level layout: header + split pane + progress. */

import { createToolbar, enableDragDrop } from './toolbar';
import { createPdfViewer } from './pdf-viewer';
import { createOutputViewer } from './output-viewer';
import { createSplitPane } from './split-pane';
import { createProgressBar } from './progress-bar';
import { initToastListener } from './toast';

export function mountApp(root: HTMLElement): void {
  root.innerHTML = '';
  root.className = 'app';

  const toolbar = createToolbar();
  const pdfViewer = createPdfViewer();
  const outputViewer = createOutputViewer();
  const splitPane = createSplitPane(pdfViewer, outputViewer);
  const progressBar = createProgressBar();

  root.append(toolbar, progressBar, splitPane);

  enableDragDrop();
  initToastListener();
}
