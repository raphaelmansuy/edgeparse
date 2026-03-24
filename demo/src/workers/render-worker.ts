/**
 * render-worker.ts — converts Markdown → HTML off the main thread.
 *
 * marked.parse() is synchronous and can take 50–300 ms on large documents.
 * Running it in a worker keeps the UI fully responsive while rendering.
 */
/// <reference lib="webworker" />

import { marked } from 'marked';

type RenderReq  = { id: string; markdown: string };
type RenderResp = { id: string; ok: true; html: string }
                | { id: string; ok: false; error: string };

(self as DedicatedWorkerGlobalScope).onmessage = async (
  e: MessageEvent<RenderReq>,
) => {
  const { id, markdown } = e.data;
  try {
    // marked.parse returns string | Promise<string> depending on version/config.
    const result = marked.parse(markdown);
    const html = result instanceof Promise ? await result : result;
    (self as DedicatedWorkerGlobalScope).postMessage(
      { id, ok: true, html } satisfies RenderResp,
    );
  } catch (err: unknown) {
    (self as DedicatedWorkerGlobalScope).postMessage(
      { id, ok: false, error: String(err) } satisfies RenderResp,
    );
  }
};
