// SPDX-License-Identifier: AGPL-3.0

import init, {
  Registry,
  compact_ref,
  convert as convertValue,
  version,
} from "./regedited_web.js";

let initialization;

function wasmCall(operation) {
  try {
    return operation();
  } catch (error) {
    throw error instanceof Error ? error : new Error(String(error));
  }
}

// Share in-flight initialization; a failed download can be retried.
export function initRegedited(wasm) {
  initialization ??= Promise.resolve()
    .then(() => init(wasm === undefined ? undefined : { module_or_path: wasm }))
    .catch((error) => {
      initialization = undefined;
      throw error instanceof Error ? error : new Error(String(error));
    });
  return initialization;
}

const aliases = new Map([
  ["l", "list"],
  ["s", "scan"],
  ["f", "fgrep"],
  ["rg", "ref-get"],
  ["ri", "resolve-index"],
  ["ist", "index-str-list"],
  ["hl", "hexline"],
  ["co", "content"],
  ["cv", "convert"],
]);

export class RegeditedRunner {
  #registry;

  constructor(content) {
    this.rawContent = String(content);
    this.#registry = wasmCall(() => new Registry(this.rawContent));
  }

  #call(method, ...args) {
    if (!this.#registry) throw new Error("RegeditedRunner has been disposed.");
    return wasmCall(() => this.#registry[method](...args));
  }

  replaceContent(content) {
    if (!this.#registry) throw new Error("RegeditedRunner has been disposed.");
    const text = String(content);
    const next = wasmCall(() => new Registry(text));
    this.#registry.free();
    this.#registry = next;
    this.rawContent = text;
    return this;
  }

  dispose() {
    this.#registry?.free();
    this.#registry = undefined;
    this.rawContent = "";
  }

  free() {
    this.dispose();
  }

  version() {
    return version();
  }

  scan() {
    return JSON.parse(this.#call("scan"));
  }

  list() {
    return this.scan().sections;
  }

  grep(pattern, scope) {
    return JSON.parse(this.#call("grep", String(pattern), scope));
  }

  fgrep(pattern, scope) {
    return this.grep(pattern, scope);
  }

  readIndex(index) {
    return JSON.parse(this.#call("read_index", typeof index === "bigint" ? String(index) : index));
  }

  resolveIndex(index) {
    return this.readIndex(index);
  }

  db(index) {
    return this.readIndex(index).db;
  }

  dbExact(index) {
    return this.readIndex(index).dbExact;
  }

  indexStrList(index) {
    return this.readIndex(index).strings;
  }

  hexline(index) {
    return this.readIndex(index).hexLine;
  }

  content(index) {
    return this.readIndex(index).content;
  }

  sectionContent(key) {
    return this.#call("section_content", String(key));
  }

  sectionData(key) {
    return this.#call("section_data", String(key));
  }

  compactRef(value) {
    return compact_ref(String(value));
  }

  convert(values, defaultType = "markdown") {
    const input = Array.isArray(values) ? values.join(" ") : String(values);
    return wasmCall(() => convertValue(input, defaultType));
  }

  refText(reference) {
    return this.#call("ref_text", String(reference));
  }

  refGet(reference) {
    const canonical = this.compactRef(reference);
    if (/^index:\d+:(zone|zonehex):\d+$/.test(canonical)) return this.refText(canonical);
    const match = /^index:(\d+)(?::(string|db):(\d+)|:(dbline|hexline|hex-word-line))?$/.exec(
      canonical,
    );
    if (!match) {
      throw new Error(`Browser ref-get does not support '${canonical}'.`);
    }

    if (match[2]) {
      const max = match[2] === "string" ? 3 : 9;
      const slot = Number(match[3]);
      if (slot < 1 || slot > max) {
        throw new RangeError(`${match[2]} slot must be between 1 and ${max}.`);
      }
    }

    const data = this.readIndex(match[1]);
    if (!match[2] && !match[4]) return data;
    if (match[2] === "string") return data.strings[Number(match[3]) - 1];
    if (match[2] === "db") return data.db[Number(match[3]) - 1];
    if (match[4] === "dbline") return data.db;
    return data.hexLine;
  }

  run(command, ...args) {
    const canonical = aliases.get(command) ?? command;
    switch (canonical) {
      case "list":
        return this.list();
      case "scan":
        return this.scan();
      case "fgrep":
        return this.fgrep(args[0], args[1]);
      case "ref-get":
        return this.refGet(args[0]);
      case "resolve-index":
        return this.resolveIndex(args[0]);
      case "index-str-list":
        return this.indexStrList(args[0]);
      case "db":
        return this.db(args[0]);
      case "db-exact":
        return this.dbExact(args[0]);
      case "hexline":
        return this.hexline(args[0]);
      case "content":
        return this.content(args[0]);
      case "convert":
        return this.convert(args[0], args[1]);
      default:
        throw new Error(
          `Browser runner command '${command}' is unavailable. The browser package exposes read-only document operations only.`,
        );
    }
  }
}

export async function createRegeditedRunner(content, { wasm } = {}) {
  await initRegedited(wasm);
  const text = typeof content?.text === "function" ? await content.text() : content;
  return new RegeditedRunner(text);
}

export async function createPageRunner({ raw = false, wasm, signal } = {}) {
  if (!globalThis.document?.documentElement) {
    throw new Error("createPageRunner requires a browser document; use createRegeditedRunner in workers.");
  }
  const content = raw
    ? await fetch(globalThis.location.href, { cache: "no-store", signal }).then((response) => {
        if (!response.ok) {
          throw new Error(`Could not read page source: HTTP ${response.status}`);
        }
        return response.text();
      })
    : globalThis.document.documentElement.outerHTML;
  return createRegeditedRunner(content, { wasm });
}

// Fetch once, then use the same local Wasm API. No polling or native execution.
export async function createLocalRunner(baseUrl = "http://localhost:5000", { wasm, signal, headers } = {}) {
  const url = new URL(baseUrl);
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) {
    throw new TypeError("Registry server URL must use HTTP(S) without embedded credentials.");
  }
  url.pathname = `${url.pathname.replace(/\/+$/, "")}/document`;
  url.search = "";
  url.hash = "";
  const response = await fetch(url, {
    method: "GET", mode: "cors", credentials: "omit", cache: "no-store", signal, headers,
  });
  if (!response.ok) throw new Error(`Could not read registry document: HTTP ${response.status}`);
  if (response.headers.get("content-type")?.split(";")[0].trim() !== "text/plain") {
    throw new Error("Expected a text/plain registry document. Update the server or check its base URL.");
  }
  return createRegeditedRunner(await response.text(), { wasm });
}
