import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { createRegeditedRunner, initRegedited } from "../pkg/runner.js";
import { bindRegistry, formatTemplate } from "../pkg/dom.js";

await initRegedited(await readFile(new URL("../pkg/regedited_web_bg.wasm", import.meta.url)));
const source = [
  "regedited open", "index: 38",
  "1x0000008 : 1x0000009 : 0x0000000 : 0x0000000 : 0x0000000 : 0x0000000",
  "1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9007199254740993",
  "Turtle <&>", "literal %s {{i38db1}}", "three", "---",
  "Hello %s", "Count: %s / {{$name}}", "outside the zone",
].join("\r\n");

test("text refs use native zone extraction and exact DB values", async () => {
  const rgd = await createRegeditedRunner(source);
  try {
    assert.equal(rgd.refText("i38s1"), "Turtle <&>");
    assert.equal(rgd.refText("i38db9"), "9007199254740993");
    assert.equal(typeof rgd.refGet("i38db9"), "number"); // existing API unchanged
    assert.equal(rgd.refGet("i38z1"), "Hello %s\r\nCount: %s / {{$name}}\r\n");
    assert.equal(rgd.refText("i38z2"), "");
    assert.equal(rgd.refGet("i38zh1"), "1x0000008 : 1x0000009");
    for (const ref of ["i38z0", "index:38:zone:4", "index:38:string:0", "index:38:db:10", "i99s1", "i38"]) {
      assert.throws(() => rgd.refText(ref), Error);
    }
    rgd.replaceContent(source.replace("1x0000009", "1x0000FFF"));
    assert.throws(() => rgd.refText("i38z1"), Error);
    rgd.replaceContent(source.replace("1x0000009", "1x0000001"));
    assert.throws(() => rgd.refText("i38z1"), Error);
  } finally { rgd.dispose(); }
});

test("zone templates accept positional refs and dynamic named values", async () => {
  const rgd = await createRegeditedRunner(source);
  try {
    assert.equal(formatTemplate(rgd, rgd.refText("i38z1"), {
      args: ["i38s1", "i38db9"], variables: { name: "viewer" },
    }), "Hello Turtle <&>\r\nCount: 9007199254740993 / viewer\r\n");
    assert.equal(formatTemplate(rgd, "{{i38s1}} | {{ index:38:db:9 }} | {{$flag}} | {{$empty}}", {
      variables: { flag: false, empty: null },
    }), "Turtle <&> | 9007199254740993 | false | ");
    assert.equal(formatTemplate(rgd, "%%s %s {{unrelated}}", { args: ["i38s2"] }),
      "%s literal %s {{i38db1}} {{unrelated}}");
    assert.equal(formatTemplate(rgd, "<b>%s</b>", { args: ["i38s1"], escapeHTML: true }),
      "<b>Turtle &lt;&amp;&gt;</b>");
    assert.equal(formatTemplate(rgd, "{{$имя}}", { variables: { имя: "Черепаха" } }), "Черепаха");
  } finally { rgd.dispose(); }
});

test("missing or mismatched arguments fail; variables never evaluate expressions", async () => {
  const rgd = await createRegeditedRunner(source);
  try {
    assert.throws(() => formatTemplate(rgd, "%s"), /Missing argument/);
    assert.throws(() => formatTemplate(rgd, "none", { args: ["i38s1"] }), /More template arguments/);
    assert.throws(() => formatTemplate(rgd, "{{$constructor}}"), /not defined/);
    assert.throws(() => formatTemplate(rgd, "%s", { args: ["$fn"], variables: { fn() {} } }), TypeError);
    assert.throws(() => formatTemplate(rgd, "%s", { args: [null] }), TypeError);
    assert.equal(formatTemplate(rgd, "{{1+1}}"), "{{1+1}}");
    assert.throws(() => bindRegistry(rgd), /DOM root/);
  } finally { rgd.dispose(); }
});

test("repeated refs are cached within one template render, not across renders", () => {
  let calls = 0;
  const rgd = { refText() { return String(++calls); } };
  assert.equal(formatTemplate(rgd, "{{i38s1}} {{i38s1}} %s", { args: ["i38s1"] }), "1 1 1");
  assert.equal(formatTemplate(rgd, "{{i38s1}}"), "2");
});
