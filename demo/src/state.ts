/** Central state store using EventTarget for reactive updates. */

import type {
  ContentElement,
  OutputFormat,
  PdfDocument,
  WasmStatus,
  ParseStatus,
  SemanticType,
} from './types';
import type { FormatCache } from './services/wasm-bridge';

export interface AppState {
  pdfBytes: Uint8Array | null;
  fileName: string;
  pageCount: number;
  currentPage: number;
  parsedDocument: PdfDocument | null;
  outputFormat: OutputFormat;
  outputText: string;
  formatCache: FormatCache | null;
  hoveredElement: ContentElement | null;
  wasmStatus: WasmStatus;
  parseStatus: ParseStatus;
  errorMessage: string | null;
  showOverlay: boolean;
  activeSemanticFilters: Set<SemanticType>;
  darkMode: boolean;
}

type StateKey = keyof AppState;

class StateStore extends EventTarget {
  private state: AppState;

  constructor() {
    super();
    this.state = {
      pdfBytes: null,
      fileName: '',
      pageCount: 0,
      currentPage: 1,
      parsedDocument: null,
      outputFormat: 'json',
      outputText: '',
      formatCache: null,
      hoveredElement: null,
      wasmStatus: 'idle',
      parseStatus: 'idle',
      errorMessage: null,
      showOverlay: true,
      activeSemanticFilters: new Set([
        'TextBlock', 'Heading', 'Table', 'TableBorder',
        'Figure', 'Image', 'List', 'Other',
        // 'Line' and 'LineArt' are off by default (too noisy on most documents)
      ]),
      darkMode: false,
    };
  }

  get<K extends StateKey>(key: K): AppState[K] {
    return this.state[key];
  }

  set<K extends StateKey>(key: K, value: AppState[K]): void {
    this.state[key] = value;
    this.dispatchEvent(new CustomEvent('change', { detail: { key, value } }));
  }

  subscribe(key: StateKey, callback: (value: unknown) => void): () => void {
    const handler = (e: Event) => {
      const { key: k, value } = (e as CustomEvent).detail;
      if (k === key) callback(value);
    };
    this.addEventListener('change', handler);
    return () => this.removeEventListener('change', handler);
  }
}

export const store = new StateStore();
