declare module 'edgeparse-wasm' {
  const init: (module_or_path?: unknown) => Promise<unknown>;
  export default init;
  export function version(): string;
  export function convert(
    pdf_bytes: Uint8Array,
    format?: string | null,
    pages?: string | null,
    reading_order?: string | null,
    table_method?: string | null,
  ): unknown;
  export function convert_to_string(
    pdf_bytes: Uint8Array,
    format?: string | null,
    pages?: string | null,
    reading_order?: string | null,
    table_method?: string | null,
  ): string;
  export class ParseSession {
    static open(
      bytes: Uint8Array,
      opts: Record<string, unknown>,
      on_progress: ((phase: string, done: number, total: number) => void) | null,
    ): ParseSession;
    candidates(): Array<{
      id: number;
      page: number;
      width: number;
      height: number;
      hash: string;
    }>;
    candidate_gray(id: number): Uint8Array;
    provide_ocr(id: number, words: unknown): void;
    finish(format: string): string;
    finishAll(): {
      json: string;
      markdown: string;
      html: string;
      text: string;
    };
    finish_hybrid(backend_md: string | null | undefined, format: string): string;
  }
}
