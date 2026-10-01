/** Greedy CTC decode matching ppu-paddle-ocr / PP-OCRv6. */

export const BLANK_INDEX = 0;
export const UNK_TOKEN = '<unk>';

/** Parse a PP-OCR dictionary text file (one char per line, blank is implicit index 0). */
export function parseDictionary(text: string): string[] {
  const lines = text.replace(/^\uFEFF/, '').split(/\r?\n/);
  // Paddle dict files list characters starting at index 1 (blank = 0).
  const chars = lines.filter((l) => l.length > 0);
  return ['', ...chars];
}

/**
 * Greedy CTC decode of a [seqLen, numClasses] logit (or probability) row.
 * Returns text + mean confidence of emitted characters.
 */
export function ctcGreedyDecode(
  logits: ArrayLike<number>,
  sequenceLength: number,
  numClasses: number,
  charDict: string[],
): { text: string; confidence: number } {
  const dictLen = charDict.length;
  const emitted: string[] = [];
  let lastCharIndex = -1;
  let confidenceSum = 0;
  let confidenceCount = 0;

  for (let t = 0; t < sequenceLength; t++) {
    const base = t * numClasses;
    let maxProb = logits[base] ?? 0;
    let maxIndex = 0;
    for (let c = 1; c < numClasses; c++) {
      const prob = logits[base + c] ?? 0;
      if (prob > maxProb) {
        maxProb = prob;
        maxIndex = c;
      }
    }
    if (maxIndex === BLANK_INDEX || maxIndex === lastCharIndex) {
      lastCharIndex = maxIndex;
      continue;
    }
    if (maxIndex >= 0 && maxIndex < dictLen) {
      const char = charDict[maxIndex] ?? '';
      if (char && char !== UNK_TOKEN) {
        emitted.push(char === '▁' ? ' ' : char);
        confidenceSum += maxProb;
        confidenceCount += 1;
      }
    }
    lastCharIndex = maxIndex;
  }

  // Collapse consecutive spaces
  const text = emitted.join('').replace(/ {2,}/g, ' ').trim();
  const confidence =
    confidenceCount > 0 ? (confidenceSum / confidenceCount) * 100 : 0;
  return { text, confidence };
}
