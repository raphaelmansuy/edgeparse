import { describe, expect, it } from 'vitest';
import { ctcGreedyDecode, parseDictionary } from '../src/ocr/ppocr/ctc-decode.js';
import {
  boxesFromProbabilityMap,
  scaleBoxesToOriginal,
} from '../src/ocr/ppocr/db-postprocess.js';

describe('parseDictionary', () => {
  it('prepends blank at index 0', () => {
    const dict = parseDictionary('a\nb\nc\n');
    expect(dict[0]).toBe('');
    expect(dict[1]).toBe('a');
    expect(dict[3]).toBe('c');
  });
});

describe('ctcGreedyDecode', () => {
  it('decodes a simple non-blank sequence', () => {
    // dict: '' blank, 'h','e','l','o'
    const dict = ['', 'h', 'e', 'l', 'o'];
    const numClasses = 5;
    // sequence: h, e, blank, l, l, o  → "helo" (CTC collapses l,l)
    const seq = [1, 2, 0, 3, 3, 4];
    const logits = new Float32Array(seq.length * numClasses);
    for (let t = 0; t < seq.length; t++) {
      for (let c = 0; c < numClasses; c++) {
        logits[t * numClasses + c] = c === seq[t] ? 0.9 : 0.01;
      }
    }
    const out = ctcGreedyDecode(logits, seq.length, numClasses, dict);
    expect(out.text).toBe('helo');
    expect(out.confidence).toBeGreaterThan(50);
  });

  it('skips blanks and repeats', () => {
    const dict = ['', 'a', 'b'];
    const numClasses = 3;
    const seq = [0, 1, 1, 0, 2, 2, 0];
    const logits = new Float32Array(seq.length * numClasses);
    for (let t = 0; t < seq.length; t++) {
      for (let c = 0; c < numClasses; c++) {
        logits[t * numClasses + c] = c === seq[t] ? 1 : 0;
      }
    }
    expect(ctcGreedyDecode(logits, seq.length, numClasses, dict).text).toBe('ab');
  });
});

describe('boxesFromProbabilityMap', () => {
  it('finds a rectangular blob', () => {
    const w = 16;
    const h = 16;
    const probs = new Float32Array(w * h);
    // 6x5 blob at (4,5)
    for (let y = 5; y < 10; y++) {
      for (let x = 4; x < 10; x++) {
        probs[y * w + x] = 0.9;
      }
    }
    const boxes = boxesFromProbabilityMap(probs, w, h, {
      threshold: 0.5,
      minArea: 4,
      padRatio: 0,
    });
    expect(boxes.length).toBe(1);
    expect(boxes[0]!.x).toBe(4);
    expect(boxes[0]!.y).toBe(5);
    expect(boxes[0]!.width).toBe(6);
    expect(boxes[0]!.height).toBe(5);
  });

  it('scales boxes back to original', () => {
    const scaled = scaleBoxesToOriginal(
      [{ x: 10, y: 20, width: 30, height: 40 }],
      0.5,
      200,
      200,
    );
    expect(scaled[0]).toEqual({ x: 20, y: 40, width: 60, height: 80 });
  });
});
