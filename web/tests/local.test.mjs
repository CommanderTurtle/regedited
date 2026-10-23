import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { createLocalRunner, initRegedited } from "../pkg/runner.js";

await initRegedited(await readFile(new URL("../pkg/regedited_web_bg.wasm", import.meta.url)));
const documentText = "世界\r\nregedited open\r\nindex: 8\r\n" +
  Array(6).fill("0x0000000").join(" : ") + "\r\n1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9\r\none\r\ntwo\r\nthree\r\n";

test("local loader is one explicit, abortable CORS fetch, including proxy prefixes", async () => {
  const previous = globalThis.fetch;
  let calls = 0;
  const controller = new AbortController();
  globalThis.fetch = async (url, options) => {
    calls++;
    assert.equal(String(url), "https://registry.example/proxy/document");
    assert.equal(options.mode, "cors");
    assert.equal(options.credentials, "omit");
    assert.equal(options.signal, controller.signal);
    assert.equal(options.headers.Authorization, "Bearer test");
    return new Response(documentText, { headers: { "content-type": "text/plain; charset=utf-8" } });
  };
  try {
    const rgd = await createLocalRunner("https://registry.example/proxy/", {
      signal: controller.signal, headers: { Authorization: "Bearer test" },
    });
    try {
      assert.equal(rgd.rawContent, documentText);
      assert.equal(rgd.refGet("i8s1"), "one");
      assert.equal(rgd.list().length, 1);
      assert.equal(calls, 1);
    } finally { rgd.dispose(); }
    await assert.rejects(createLocalRunner("file:///registry.txt"), TypeError);
    await assert.rejects(createLocalRunner("http://user:password@localhost:5000"), TypeError);
    assert.equal(calls, 1);
    globalThis.fetch = async () => new Response("not found", { status: 404 });
    await assert.rejects(createLocalRunner(), /HTTP 404/);
    globalThis.fetch = async () => Response.json({ status: "ok" });
    await assert.rejects(createLocalRunner(), /text\/plain/);
  } finally { globalThis.fetch = previous; }
});

test("native server serves exact snapshots, CORS/preflight and existing routes", { timeout: 20000 }, async () => {
  const binary = process.env.REGEDITED_BIN || fileURLToPath(new URL(
    `../../target/debug/regedited${process.platform === "win32" ? ".exe" : ""}`, import.meta.url,
  ));
  const dir = await mkdtemp(join(tmpdir(), "regedited-web-test-"));
  const file = join(dir, "registry.txt");
  try {
    await writeFile(file, documentText);
    for (const cors of [true, false]) {
      const reservation = createServer();
      await new Promise((resolve, reject) => { reservation.once("error", reject); reservation.listen(0, "127.0.0.1", resolve); });
      const port = reservation.address().port;
      await new Promise((resolve) => reservation.close(resolve));
      const child = spawn(binary, ["serve", "--file", file, "--port", String(port), ...(cors ? ["--cors"] : [])], {
        windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
      });
      let output = "", spawnError;
      child.on("error", (error) => { spawnError = error; });
      child.stdout.on("data", (data) => { output += data; });
      child.stderr.on("data", (data) => { output += data; });
      const base = `http://127.0.0.1:${port}`;
      try {
        let ready = false;
        for (let attempt = 0; attempt < 100; attempt++) {
          if (spawnError) throw spawnError;
          if (child.exitCode !== null) throw new Error(`Server exited: ${output}`);
          try { ready = (await fetch(`${base}/health`)).ok; } catch {}
          if (ready) break;
          await new Promise((resolve) => setTimeout(resolve, 30));
        }
        assert(ready, `Server did not start: ${output}`);
        const response = await fetch(`${base}/document`, { headers: { Origin: "https://example.com" } });
        assert.equal(await response.text(), documentText);
        assert.equal(response.headers.get("access-control-allow-origin"), cors ? "*" : null);
        assert.equal(response.headers.get("access-control-allow-credentials"), null);
        assert.equal(response.headers.get("cache-control"), "no-store");
        const preflight = await fetch(`${base}/document`, { method: "OPTIONS" });
        assert.equal(preflight.status, cors ? 204 : 404);
        const missing = await fetch(`${base}/missing`);
        assert.equal(missing.headers.get("access-control-allow-origin"), cors ? "*" : null);
        const query = await fetch(`${base}/query`, {
          method: "POST", headers: { "Content-Type": "application/json" }, body: "{}",
        });
        assert.equal(query.status, 400);
        assert.equal(query.headers.get("access-control-allow-origin"), cors ? "*" : null);
        assert.equal((await fetch(`${base}/sections`)).status, 200);
        if (cors) {
          const rgd = await createLocalRunner(base);
          try { assert.equal(rgd.refGet("i8s2"), "two"); } finally { rgd.dispose(); }
          const controller = new AbortController();
          controller.abort();
          await assert.rejects(createLocalRunner(base, { signal: controller.signal }), { name: "AbortError" });
        }
      } finally {
        if (child.exitCode === null && !spawnError) {
          const stopped = new Promise((resolve) => child.once("exit", resolve));
          child.kill();
          await stopped;
        }
      }
    }
  } finally { await rm(dir, { recursive: true, force: true }); }
});
