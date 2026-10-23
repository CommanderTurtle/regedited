// SPDX-License-Identifier: AGPL-3.0
import type { InitInput, InitOutput } from "./regedited_web.js";

export type Index = number | string | bigint;
export type WasmSource = InitInput | Promise<InitInput>;
export interface RunnerOptions { wasm?: WasmSource }
export interface PageOptions extends RunnerOptions { raw?: boolean; signal?: AbortSignal }
export interface LocalOptions extends RunnerOptions { signal?: AbortSignal; headers?: HeadersInit }

export interface Section {
  name: string;
  headerLine: number;
  indexLine: number;
  hexLine: number;
  numericLine: number;
  stringLines: [number, number, number];
  recordStart: number;
  recordEnd: number;
  recordLines: number;
  headerByteOffset: number;
}
export interface ScanResult { totalLines: number; totalBytes: number; sections: Section[] }
export interface GrepResult {
  pattern: string;
  scope: string | null;
  count: number;
  matches: { line: number; lineNumber: number; text: string }[];
}
export interface IndexResult {
  /** Use indexExact/key for indexes beyond Number.MAX_SAFE_INTEGER. */
  index: number;
  indexExact: string;
  key: string;
  hexLine: string;
  db: number[];
  dbExact: string[];
  strings: [string, string, string];
  content: string;
  recordStart: number;
  recordEnd: number;
}
export type RefValue = IndexResult | string | number | number[];

export function initRegedited(wasm?: WasmSource): Promise<InitOutput>;
/** Await initRegedited first, or use createRegeditedRunner. */
export class RegeditedRunner {
  constructor(content: string);
  rawContent: string;
  version(): string;
  scan(): ScanResult;
  list(): Section[];
  grep(pattern: string, scope?: string): GrepResult;
  fgrep(pattern: string, scope?: string): GrepResult;
  readIndex(index: Index): IndexResult;
  resolveIndex(index: Index): IndexResult;
  db(index: Index): number[];
  dbExact(index: Index): string[];
  indexStrList(index: Index): [string, string, string];
  hexline(index: Index): string;
  content(index: Index): string;
  sectionContent(key: string): string;
  sectionData(key: string): string;
  compactRef(value: string): string;
  convert(values: string | readonly (string | number)[], defaultType?: string): string;
  refGet(reference: string): RefValue;
  /** Exact decimal text, string, zone content or zone bounds. No Number rounding. */
  refText(reference: string): string;
  run(command: "scan" | "s"): ScanResult;
  run(command: "list" | "l"): Section[];
  run(command: "fgrep" | "f", pattern: string, scope?: string): GrepResult;
  run(command: "resolve-index" | "ri", index: Index): IndexResult;
  run(command: "db", index: Index): number[];
  run(command: "db-exact", index: Index): string[];
  run(command: "index-str-list" | "ist", index: Index): [string, string, string];
  run(command: "hexline" | "hl" | "content" | "co", index: Index): string;
  run(command: "ref-get" | "rg", reference: string): RefValue;
  run(command: "convert" | "cv", values: string | readonly (string | number)[], defaultType?: string): string;
  replaceContent(content: string): this;
  /** Release the Rust document immediately. Safe to call more than once. */
  dispose(): void;
  free(): void;
}
export function createRegeditedRunner(content: string | Blob, options?: RunnerOptions): Promise<RegeditedRunner>;
export function createPageRunner(options?: PageOptions): Promise<RegeditedRunner>;
/** One snapshot from a Regedited server started with --cors. Does not poll. */
export function createLocalRunner(baseUrl?: string | URL, options?: LocalOptions): Promise<RegeditedRunner>;
