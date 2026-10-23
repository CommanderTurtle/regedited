// SPDX-License-Identifier: AGPL-3.0

const selector = "[data-rgd-ref], [data-rgd-template]";
const forbidden = new Set(["SCRIPT", "STYLE", "IFRAME", "OBJECT", "EMBED", "LINK", "META", "BASE"]);
const tokens = /%%|%s|\{\{\s*((?:i\d[^{}\s]*|index:\d[^{}\s]*|\$[^{}\s]+))\s*\}\}/g;
const escapeHTML = (text) => text.replace(/[&<>"']/g, (char) => ({
  "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
})[char]);

function text(value) {
  if (value === null) return "";
  if (!["string", "number", "bigint", "boolean"].includes(typeof value)) {
    throw new TypeError("Template values must be strings, numbers, bigints, booleans or null.");
  }
  return String(value);
}

function resolver(runner, variables) {
  const cache = new Map();
  return (reference) => {
    if (typeof reference !== "string" || !reference.trim()) {
      throw new TypeError("Template arguments must be non-empty reference strings.");
    }
    reference = reference.trim();
    if (reference.startsWith("$")) {
      const name = reference.slice(1);
      if (!Object.prototype.hasOwnProperty.call(variables, name)) {
        throw new Error(`Template variable '${name}' is not defined.`);
      }
      return text(variables[name]);
    }
    if (!cache.has(reference)) cache.set(reference, runner.refText(reference));
    return cache.get(reference);
  };
}

function format(template, args, resolve, html) {
  if (!Array.isArray(args)) throw new TypeError("Template args must be an array of references.");
  let position = 0;
  const result = String(template).replace(tokens, (token, reference) => {
    if (token === "%%") return "%";
    if (token === "%s" && position >= args.length) throw new Error("Missing argument for %s.");
    const value = resolve(token === "%s" ? args[position++] : reference);
    return html ? escapeHTML(value) : value;
  });
  if (position !== args.length) throw new Error("More template arguments than %s placeholders.");
  return result;
}

/** Data-only substitution: no eval, expressions or recursive expansion. */
export function formatTemplate(runner, template, { args = [], variables = {}, escapeHTML = false } = {}) {
  return format(template, args, resolver(runner, variables), escapeHTML);
}

/** Bind only explicitly marked elements; the outermost binding owns its subtree. */
export function bindRegistry(runner, root = globalThis.document, { variables = {}, sanitizeHTML } = {}) {
  if (!root?.querySelectorAll) throw new TypeError("bindRegistry requires a DOM root.");
  let currentVariables = { ...variables };
  const view = {
    render(variables = {}) {
      if (!root) throw new Error("Registry binding has been disposed.");
      const nextVariables = { ...currentVariables, ...variables };
      const resolve = resolver(runner, nextVariables);
      const candidates = new Set([
        ...(root.matches?.(selector) ? [root] : []), ...root.querySelectorAll(selector),
      ]);
      const plans = [];
      for (const element of candidates) {
        let parent = element.parentElement;
        while (parent && !candidates.has(parent)) parent = parent.parentElement;
        if (parent) continue;
        if (forbidden.has(element.tagName)) throw new Error(`Cannot bind ${element.tagName} elements.`);
        const reference = element.getAttribute("data-rgd-ref");
        const template = element.getAttribute("data-rgd-template");
        if (reference !== null && template !== null) throw new Error("Use data-rgd-ref or data-rgd-template, not both.");
        const source = resolve(reference ?? template);
        const mode = element.getAttribute("data-rgd-mode") || "text";
        if (!["text", "html", "value"].includes(mode)) throw new Error(`Unknown registry binding mode '${mode}'.`);
        if (mode === "value" && !("value" in element)) throw new Error("Value bindings need a form control.");
        if (mode === "value" && element.tagName === "INPUT" && element.type === "file") {
          throw new Error("File input values cannot be bound programmatically.");
        }
        const rawArgs = element.getAttribute("data-rgd-args");
        if (reference !== null && rawArgs !== null) throw new Error("%s arguments require data-rgd-template.");
        let args = [];
        if (rawArgs !== null) {
          try { args = JSON.parse(rawArgs); }
          catch { throw new Error("data-rgd-args must be a JSON array of references."); }
        }
        const output = template !== null ? format(source, args, resolve, mode === "html") : source;
        let fragment;
        if (mode === "html") {
          if (typeof sanitizeHTML !== "function") throw new Error("HTML bindings require a sanitizeHTML callback.");
          const safe = sanitizeHTML(output);
          if (typeof safe !== "string" && !element.ownerDocument.defaultView?.trustedTypes?.isHTML(safe)) {
            throw new TypeError("sanitizeHTML must synchronously return a string or TrustedHTML.");
          }
          const staging = element.ownerDocument.createElement("template");
          staging.innerHTML = safe;
          fragment = staging.content;
        }
        plans.push({ element, mode, output, fragment });
      }
      // Resolve and validate all bindings before touching the live document.
      for (const { element, mode, output, fragment } of plans) {
        if (mode === "html") element.replaceChildren(fragment);
        else if (mode === "value") element.value = output;
        else element.textContent = output;
      }
      currentVariables = nextVariables;
      return plans.length;
    },
    dispose() {
      root = undefined;
      runner = undefined;
      currentVariables = {};
      variables = undefined;
      sanitizeHTML = undefined;
    },
  };
  view.render();
  return view;
}
