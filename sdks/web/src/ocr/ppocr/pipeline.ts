/**
 * PP-OCRv6 detect → recognize pipeline.
 * ORT is injected so the same code runs under onnxruntime-web and onnxruntime-node.
 */

import { ctcGreedyDecode, parseDictionary } from './ctc-decode.js';
import { boxesFromProbabilityMap, scaleBoxesToOriginal } from './db-postprocess.js';
import { prepareDetection, prepareRecognition, type Box } from './preprocess.js';
import type { OcrWord } from '../types.js';

export interface OrtTensor {
  data: Float32Array | Int32Array | Uint8Array;
  dims: number[];
  dispose?: () => void;
}

export interface OrtSession {
  inputNames: string[];
  outputNames: string[];
  run(feeds: Record<string, OrtTensor>): Promise<Record<string, OrtTensor>>;
}

export interface OrtLike {
  Tensor: new (
    type: string,
    data: Float32Array,
    dims: number[],
  ) => OrtTensor;
  InferenceSession: {
    create(
      model: Uint8Array | ArrayBuffer,
      opts?: { executionProviders?: string[] },
    ): Promise<OrtSession>;
  };
}

export interface PpocrPipelineOptions {
  detModel: Uint8Array;
  recModel: Uint8Array;
  dictionaryText: string;
  ort: OrtLike;
  providers?: string[];
  maxDetSide?: number;
  maxBoxes?: number;
}

export class PpocrPipeline {
  private det: OrtSession | null = null;
  private rec: OrtSession | null = null;
  private dict: string[] = [];
  private readonly ort: OrtLike;
  private readonly providers: string[];
  private readonly maxDetSide: number;
  private readonly maxBoxes: number;
  private readonly detBytes: Uint8Array;
  private readonly recBytes: Uint8Array;
  private readonly dictionaryText: string;

  constructor(opts: PpocrPipelineOptions) {
    this.ort = opts.ort;
    this.providers = opts.providers ?? ['wasm'];
    this.maxDetSide = opts.maxDetSide ?? 960;
    this.maxBoxes = opts.maxBoxes ?? 200;
    this.detBytes = opts.detModel;
    this.recBytes = opts.recModel;
    this.dictionaryText = opts.dictionaryText;
  }

  async ready(): Promise<void> {
    if (this.det && this.rec) return;
    this.dict = parseDictionary(this.dictionaryText);
    this.det = await this.ort.InferenceSession.create(this.detBytes, {
      executionProviders: this.providers,
    });
    this.rec = await this.ort.InferenceSession.create(this.recBytes, {
      executionProviders: this.providers,
    });
  }

  async recognizeGray(
    gray: Uint8Array,
    width: number,
    height: number,
  ): Promise<OcrWord[]> {
    await this.ready();
    if (!this.det || !this.rec) return [];

    const det = prepareDetection(gray, width, height, this.maxDetSide);
    const detInputName = this.det.inputNames[0] ?? 'x';
    const detTensor = new this.ort.Tensor('float32', det.tensor, [
      1,
      3,
      det.height,
      det.width,
    ]);
    let detOut: Record<string, OrtTensor>;
    try {
      detOut = await this.det.run({ [detInputName]: detTensor });
    } finally {
      detTensor.dispose?.();
    }
    const detKey = this.det.outputNames[0] ?? Object.keys(detOut)[0]!;
    const detData = detOut[detKey]!;
    const probs = detData.data as Float32Array;
    // dims often [1,1,H,W] or [1,H,W]
    const mapH = detData.dims[detData.dims.length - 2] ?? det.height;
    const mapW = detData.dims[detData.dims.length - 1] ?? det.width;
    let boxes = boxesFromProbabilityMap(probs, mapW, mapH);
    boxes = scaleBoxesToOriginal(boxes, det.ratio, width, height);
    if (boxes.length > this.maxBoxes) {
      boxes = boxes.slice(0, this.maxBoxes);
    }

    const words: OcrWord[] = [];
    for (const box of boxes) {
      const w = await this.recognizeBox(gray, width, height, box);
      if (w) words.push(w);
    }
    return words;
  }

  private async recognizeBox(
    gray: Uint8Array,
    width: number,
    height: number,
    box: Box,
  ): Promise<OcrWord | null> {
    if (!this.rec) return null;
    if (box.width < 4 || box.height < 4) return null;
    const prep = prepareRecognition(gray, width, height, box);
    const inputName = this.rec.inputNames[0] ?? 'x';
    const tensor = new this.ort.Tensor('float32', prep.tensor, [
      1,
      3,
      prep.height,
      prep.width,
    ]);
    let out: Record<string, OrtTensor>;
    try {
      out = await this.rec.run({ [inputName]: tensor });
    } finally {
      tensor.dispose?.();
    }
    const key = this.rec.outputNames[0] ?? Object.keys(out)[0]!;
    const logits = out[key]!;
    // dims: [1, seq, classes] or [1, classes, seq]
    const dims = logits.dims;
    let seqLen: number;
    let numClasses: number;
    let data = logits.data as Float32Array;
    if (dims.length === 3 && (dims[2] ?? 0) > (dims[1] ?? 0)) {
      // [1, seq, classes]
      seqLen = dims[1] ?? 1;
      numClasses = dims[2] ?? 1;
    } else if (dims.length === 3) {
      // [1, classes, seq] — transpose mentally by reading differently
      numClasses = dims[1] ?? 1;
      seqLen = dims[2] ?? 1;
      const transposed = new Float32Array(seqLen * numClasses);
      for (let t = 0; t < seqLen; t++) {
        for (let c = 0; c < numClasses; c++) {
          transposed[t * numClasses + c] = data[c * seqLen + t] ?? 0;
        }
      }
      data = transposed;
    } else {
      seqLen = dims[1] ?? 1;
      numClasses = dims[2] ?? dims[1] ?? 1;
    }
    const decoded = ctcGreedyDecode(data, seqLen, numClasses, this.dict);
    if (!decoded.text) return null;
    return {
      text: decoded.text,
      left: box.x,
      top: box.y,
      width: box.width,
      height: box.height,
      confidence: decoded.confidence,
    };
  }

  async dispose(): Promise<void> {
    this.det = null;
    this.rec = null;
  }
}
