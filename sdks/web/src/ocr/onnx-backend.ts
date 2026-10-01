/**
 * PP-OCR ONNX backend for the OCR worker.
 */

import type { OcrBackend, OcrRequest, OcrWord } from './types.js';
import { PpocrPipeline, type OrtLike } from './ppocr/pipeline.js';

export interface OnnxOcrBackendOptions {
  models?: {
    detection?: Uint8Array;
    recognition?: Uint8Array;
    dictionary?: string;
  };
  providers?: Array<'webgpu' | 'wasm'>;
  ort?: OrtLike;
}

export class OnnxOcrBackend implements OcrBackend {
  readonly name = 'onnx-ppocr';
  private readonly opts: OnnxOcrBackendOptions;
  private pipeline: PpocrPipeline | null = null;
  private initPromise: Promise<void> | null = null;

  constructor(opts: OnnxOcrBackendOptions = {}) {
    this.opts = opts;
  }

  async ready(): Promise<void> {
    if (!this.initPromise) {
      this.initPromise = this.init();
    }
    await this.initPromise;
  }

  private async init(): Promise<void> {
    const det = this.opts.models?.detection;
    const rec = this.opts.models?.recognition;
    const dictionary = this.opts.models?.dictionary;
    if (!det || !rec || !dictionary) {
      return;
    }
    let ort = this.opts.ort;
    if (!ort) {
      try {
        ort = (await import('onnxruntime-web')) as unknown as OrtLike;
      } catch {
        return;
      }
    }
    const providers = (this.opts.providers ?? ['webgpu', 'wasm']).map(String);
    try {
      this.pipeline = new PpocrPipeline({
        detModel: det,
        recModel: rec,
        dictionaryText: dictionary,
        ort,
        providers,
      });
      await this.pipeline.ready();
    } catch {
      this.pipeline = null;
    }
  }

  async recognize(req: OcrRequest): Promise<OcrWord[]> {
    await this.ready();
    if (!this.pipeline) return [];
    try {
      return await this.pipeline.recognizeGray(req.gray, req.width, req.height);
    } catch {
      return [];
    }
  }

  async dispose(): Promise<void> {
    await this.pipeline?.dispose();
    this.pipeline = null;
  }
}

/** Stub backend used when OCR is off or models declined. */
export class NullOcrBackend implements OcrBackend {
  readonly name = 'null';
  async ready(): Promise<void> {}
  async recognize(_req: OcrRequest): Promise<OcrWord[]> {
    return [];
  }
}
