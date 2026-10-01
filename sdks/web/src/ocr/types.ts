export interface OcrWord {
  text: string;
  left: number;
  top: number;
  width: number;
  height: number;
  confidence?: number;
}

export interface OcrRequest {
  id: string;
  hash: string;
  width: number;
  height: number;
  gray: Uint8Array;
}

export interface OcrBackend {
  readonly name: string;
  ready(): Promise<void>;
  recognize(req: OcrRequest): Promise<OcrWord[]>;
  dispose?(): Promise<void>;
}

/** In-memory OCR result cache keyed by candidate content hash. */
export class OcrResultCache {
  private readonly map = new Map<string, OcrWord[]>();
  hits = 0;
  misses = 0;

  get(hash: string): OcrWord[] | undefined {
    const hit = this.map.get(hash);
    if (hit) {
      this.hits += 1;
      return hit;
    }
    this.misses += 1;
    return undefined;
  }

  set(hash: string, words: OcrWord[]): void {
    this.map.set(hash, words);
  }

  clear(): void {
    this.map.clear();
    this.hits = 0;
    this.misses = 0;
  }
}
