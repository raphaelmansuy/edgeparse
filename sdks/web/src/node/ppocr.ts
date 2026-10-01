/**
 * Node OCR backend backed by PP-OCRv6 + onnxruntime-node.
 */

import { readFile } from 'node:fs/promises';
import type { OcrBackend, OcrRequest, OcrWord } from '../ocr/types.js';
import { NullOcrBackend } from '../ocr/onnx-backend.js';
import { PpocrPipeline, type OrtLike } from '../ocr/ppocr/pipeline.js';

export interface NodePpocrOptions {
  detPath: string;
  recPath: string;
  dictPath: string;
  providers?: string[];
}

export class NodePpocrBackend implements OcrBackend {
  readonly name = 'node-ppocr';
  private pipeline: PpocrPipeline | null = null;
  private readonly opts: NodePpocrOptions;

  constructor(opts: NodePpocrOptions) {
    this.opts = opts;
  }

  async ready(): Promise<void> {
    if (this.pipeline) return;
    let ort: OrtLike;
    try {
      // optional peer — resolved at runtime in Node
      // @ts-expect-error onnxruntime-node is an optional peer dependency
      ort = (await import('onnxruntime-node')) as unknown as OrtLike;
    } catch {
      throw new Error('onnxruntime-node is required for NodePpocrBackend');
    }
    const [det, rec, dictBuf] = await Promise.all([
      readFile(this.opts.detPath),
      readFile(this.opts.recPath),
      readFile(this.opts.dictPath),
    ]);
    this.pipeline = new PpocrPipeline({
      detModel: new Uint8Array(det),
      recModel: new Uint8Array(rec),
      dictionaryText: dictBuf.toString('utf8'),
      ort,
      providers: this.opts.providers ?? ['cpu'],
    });
    await this.pipeline.ready();
  }

  async recognize(req: OcrRequest): Promise<OcrWord[]> {
    await this.ready();
    if (!this.pipeline) return [];
    return this.pipeline.recognizeGray(req.gray, req.width, req.height);
  }

  async dispose(): Promise<void> {
    await this.pipeline?.dispose();
    this.pipeline = null;
  }
}

export { NullOcrBackend };
