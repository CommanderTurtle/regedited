// SPDX-License-Identifier: AGPL-3.0
import type { RegeditedRunner } from "./runner.js";

export type TemplateValue = string | number | bigint | boolean | null;
export type TemplateVariables = Readonly<Record<string, TemplateValue>>;
export interface TemplateOptions {
  /** Each %s consumes a registry ref (i38s1, i38db1, i38z1) or $variable. */
  args?: readonly string[];
  variables?: TemplateVariables;
  escapeHTML?: boolean;
}
export interface BindingOptions {
  variables?: TemplateVariables;
  /** Synchronous sanitizer for the complete result, including URLs/attributes. Objects must be native TrustedHTML. */
  sanitizeHTML?: (html: string) => string | { toString(): string };
}
export interface RegistryBinding {
  /** Explicit refresh. Supplied variables merge with the current variables. Returns binding count. */
  render(variables?: TemplateVariables): number;
  /** Release bindings without removing their DOM output or disposing the runner. */
  dispose(): void;
}
export function formatTemplate(runner: RegeditedRunner, template: string, options?: TemplateOptions): string;
export function bindRegistry(runner: RegeditedRunner, root?: Document | Element | DocumentFragment, options?: BindingOptions): RegistryBinding;
