/* tslint:disable */
/* eslint-disable */

/**
 * Chroma subsampling format
 */
export enum ChromaSampling {
  /**
   * Both vertically and horizontally subsampled.
   */
  Cs420 = 0,
  /**
   * Horizontally subsampled.
   */
  Cs422 = 1,
  /**
   * Not subsampled.
   */
  Cs444 = 2,
  /**
   * Monochrome.
   */
  Cs400 = 3,
}

/**
 * Convert a document to a JSON string containing metadata and content.
 *
 * The returned JSON has the shape:
 * ```json
 * {
 *   "format": "hwp" | "hwpx" | "pdf" | "docx",
 *   "version": "...",
 *   "markdown": "...",
 *   "metadata": { ... }
 * }
 * ```
 */
export function convert_to_json(data: Uint8Array, filename: string): string;

/**
 * Convert a document to Markdown.
 *
 * The format is auto-detected from the filename extension and magic bytes.
 * On success the Markdown string is returned; on error a `JsValue`
 * containing the error message is thrown.
 */
export function convert_to_markdown(data: Uint8Array, filename: string): string;

/**
 * Detect the document format from the filename extension and/or magic bytes.
 *
 * Returns one of `"hwp"`, `"hwpx"`, `"pdf"`, `"docx"`, or `"unknown"`.
 */
export function detect_format(data: Uint8Array, filename: string): string;

/**
 * Return the crate version (from `Cargo.toml`).
 */
export function get_version(): string;

/**
 * Initialise `console_error_panic_hook` so Rust panics produce readable
 * stack traces in the browser console.
 */
export function init(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly convert_to_json: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly convert_to_markdown: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly detect_format: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly get_version: (a: number) => void;
  readonly init: () => void;
  readonly __wbindgen_export: (a: number, b: number, c: number) => void;
  readonly __wbindgen_export2: (a: number, b: number) => number;
  readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
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
