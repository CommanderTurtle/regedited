// SPDX-License-Identifier: AGPL-3.0

import init, {
  Registry,
  compact_ref,
  convert as convertValue,
  version,
} from "./regedited_web.js";

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
    this.#registry = new Registry(this.rawContent);
  }

  version() {
    return version();
  }

  scan() {
    return JSON.parse(this.#registry.scan());
  }

  list() {
    return this.scan().sections;
  }

  grep(pattern, scope) {
    return JSON.parse(this.#registry.grep(String(pattern), scope));
  }

  fgrep(pattern, scope) {
    return this.grep(pattern, scope);
  }

  readIndex(index) {
    return JSON.parse(this.#registry.read_index(index));
  }

  resolveIndex(index) {
    return this.readIndex(index);
  }

  db(index) {
    return this.readIndex(index).db;
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
    return this.#registry.section_content(String(key));
  }

  sectionData(key) {
    return this.#registry.section_data(String(key));
  }

  compactRef(value) {
    return compact_ref(String(value));
  }

  convert(values, defaultType = "markdown") {
    const input = Array.isArray(values) ? values.join(" ") : String(values);
    return convertValue(input, defaultType);
  }

  refGet(reference) {
    const canonical = this.compactRef(reference);
    const match = /^index:(\d+)(?::(string|db):(\d+)|:(dbline|hexline|hex-word-line))?$/.exec(
      canonical,
    );
    if (!match) {
      throw new Error(`Browser ref-get does not support '${canonical}'.`);
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

export async function createRegeditedRunner(content) {
  await init();
  return new RegeditedRunner(content);
}

export async function createPageRunner({ raw = false } = {}) {
  const content = raw
    ? await fetch(globalThis.location.href, { cache: "no-store" }).then((response) => {
        if (!response.ok) {
          throw new Error(`Could not read page source: HTTP ${response.status}`);
        }
        return response.text();
      })
    : globalThis.document.documentElement.outerHTML;
  return createRegeditedRunner(content);
}
