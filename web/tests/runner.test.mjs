import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  createPageRunner,
  createRegeditedRunner,
  initRegedited,
} from "../pkg/runner.js";

const wasm = await readFile(new URL("../pkg/regedited_web_bg.wasm", import.meta.url));
const documentText = [
  "payload 世界",
  "prefix regedited open suffix",
  "index: 38",
  Array(6).fill("0x0000000").join(" : "),
  "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9007199254740993",
  "one", "two", "three",
  "shared tail 🦀",
].join("\n");

await test("initialization retries failures and shares concurrent loads", async () => {
  await assert.rejects(initRegedited(new Uint8Array([0])), Error);
  const first = initRegedited(wasm);
  assert.equal(initRegedited(wasm), first);
  await first;
});

test("real Wasm scans, greps, reads refs and preserves exact numbers", async () => {
  const rgd = await createRegeditedRunner(documentText);
  try {
    assert.equal(rgd.list()[0].name, "index:38");
    assert.equal(rgd.run("rg", "i38s2"), "two");
    assert.equal(rgd.grep("世界").count, 1);
    assert.equal(rgd.readIndex(38).indexExact, "38");
    assert.equal(rgd.dbExact(38)[8], "9007199254740993");
    assert.match(rgd.convert("d 58 p 59"), /x/);
    assert.equal(rgd.sectionContent("index:38"), rgd.content(38));
  } finally { rgd.dispose(); }
});

test("File/Blob inputs and full u64 indexes work without a DOM", async () => {
  const text = documentText.replace("index: 38", "index: 18446744073709551615");
  const rgd = await createRegeditedRunner(new Blob([text]));
  try {
    assert.equal(rgd.readIndex(18446744073709551615n).indexExact, "18446744073709551615");
    assert.equal(rgd.refGet("i18446744073709551615s1"), "one");
    assert.throws(() => rgd.readIndex(Number.MAX_SAFE_INTEGER + 1), Error);
  } finally { rgd.dispose(); }
});

test("content replacement is atomic and disposal is idempotent", async () => {
  const rgd = await createRegeditedRunner(documentText);
  assert.throws(() => rgd.replaceContent("regedited open\n"), Error);
  assert.equal(rgd.refGet("i38s1"), "one");
  assert.equal(rgd.rawContent, documentText);
  assert.equal(rgd.replaceContent(documentText.replace("one", "updated")), rgd);
  assert.equal(rgd.refGet("i38s1"), "updated");
  rgd.dispose();
  rgd.free();
  assert.equal(rgd.rawContent, "");
  assert.throws(() => rgd.scan(), /disposed/);
  assert.throws(() => rgd.replaceContent(documentText), /disposed/);
});

test("bad inputs throw Error objects instead of undefined or raw Wasm strings", async () => {
  await assert.rejects(createRegeditedRunner("regedited open\n"), Error);
  const rgd = await createRegeditedRunner(documentText);
  try {
    for (const ref of ["index:38:string:0", "index:38:string:4", "index:38:db:10", "i99s1", "garbage"]) {
      assert.throws(() => rgd.refGet(ref), Error);
    }
    assert.throws(() => rgd.readIndex(-1), Error);
    assert.throws(() => rgd.run("delete", 38), /read-only/);
  } finally { rgd.dispose(); }
  await assert.rejects(createPageRunner(), /browser document/);
});

test("package ships the typed runner without changing the low-level entry point", async () => {
  const pkg = JSON.parse(await readFile(new URL("../pkg/package.json", import.meta.url), "utf8"));
  const cargo = await readFile(new URL("../Cargo.toml", import.meta.url), "utf8");
  assert.equal(pkg.version, /^version = "([^"]+)"/m.exec(cargo)[1]);
  assert.equal(pkg.main, "regedited_web.js");
  assert.equal(pkg.type, "module");
  assert(pkg.files.includes("runner.js"));
  assert(pkg.files.includes("runner.d.ts"));
  for (const file of pkg.files) await readFile(new URL(`../pkg/${file}`, import.meta.url));
});
