import type { Snapshot } from '../types.js';

type Listener = () => void;

/** Immutable snapshot store compatible with React `useSyncExternalStore`. */
export class StateStore {
  private snapshot: Snapshot;
  private listeners = new Set<Listener>();

  constructor(initial: Snapshot) {
    this.snapshot = Object.freeze(structuredClone(initial));
  }

  getSnapshot = (): Snapshot => this.snapshot;

  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  update(mutator: (draft: Snapshot) => void): Snapshot {
    const draft = structuredClone(this.snapshot) as Snapshot;
    mutator(draft);
    this.snapshot = Object.freeze(draft);
    for (const l of this.listeners) l();
    return this.snapshot;
  }
}

export function emptySnapshot(): Snapshot {
  return {
    engine: { state: 'idle' },
    models: {},
    jobs: {},
    capabilities: null,
    online: typeof navigator === 'undefined' ? true : navigator.onLine,
  };
}

/** Weighted monotonic progress: never decreases within a job. */
export class ProgressTracker {
  private fraction = 0;

  get current(): number {
    return this.fraction;
  }

  /** Set absolute fraction in [0, 1], clamping upward only. */
  set(next: number): number {
    const clamped = Math.min(1, Math.max(0, next));
    this.fraction = Math.max(this.fraction, clamped);
    return this.fraction;
  }

  /** Advance by a weighted slice of remaining room. */
  advance(share: number, withinShare: number): number {
    const room = 1 - this.fraction;
    const delta = room * share * Math.min(1, Math.max(0, withinShare));
    return this.set(this.fraction + delta);
  }
}

/** Job phase weights (sum ≈ 1). */
export const JOB_WEIGHTS = {
  opening: 0.05,
  planning: 0.25,
  ocr: 0.55,
  assembling: 0.15,
} as const;
