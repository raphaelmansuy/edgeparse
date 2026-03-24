/* tslint:disable */
/* eslint-disable */

/**
 * Convert PDF bytes to a structured document object (returned as JS value).
 *
 * # Arguments
 * * `pdf_bytes` — raw PDF file as `Uint8Array`
 * * `format` — output format hint: `"json"` (default) | `"markdown"` | `"html"` | `"text"`
 * * `pages` — page range: `"all"` (default) or `"1-5"` or `"1,3,7"`
 * * `reading_order` — `"auto"` (default) or `"off"`
 * * `table_method` — `"default"` (default) or `"cluster"`
 */
export function convert(pdf_bytes: Uint8Array, format?: string | null, pages?: string | null, reading_order?: string | null, table_method?: string | null): any;

/**
 * Convert PDF bytes to a formatted output string.
 *
 * # Arguments
 * * `pdf_bytes` — raw PDF file as `Uint8Array`
 * * `format` — `"json"` (default) | `"markdown"` | `"html"` | `"text"`
 * * `pages` — page range
 * * `reading_order` — `"auto"` | `"off"`
 * * `table_method` — `"default"` | `"cluster"`
 */
export function convert_to_string(pdf_bytes: Uint8Array, format?: string | null, pages?: string | null, reading_order?: string | null, table_method?: string | null): string;

/**
 * Initialize panic hook for better error messages in browser console.
 */
export function init(): void;

/**
 * Return the edgeparse version string.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly convert: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly convert_to_string: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly version: (a: number) => void;
    readonly init: () => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export4: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
