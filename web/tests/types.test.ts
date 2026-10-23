import { createRegeditedRunner, createLocalRunner, initRegedited } from "../pkg/runner.js";
import { bindRegistry, formatTemplate } from "../pkg/dom.js";

async function usage(file: File, wasm: WebAssembly.Module, signal: AbortSignal) {
  await initRegedited(wasm);
  const rgd = await createRegeditedRunner(file);
  const exact: string[] = rgd.run("db-exact", 38n);
  const numbers: number[] = rgd.run("db", "38");
  const key: string = rgd.readIndex(38).indexExact;
  const text: string = rgd.run("cv", ["d", 58, "p", 59]);
  const rendered: string = formatTemplate(rgd, "%s {{$name}}", { args: ["i38s1"], variables: { name: "Turtle" } });
  const binding = bindRegistry(rgd, document, { variables: { name: "Turtle" } });
  const count: number = binding.render({ name: "Updated" });
  binding.dispose();
  rgd.replaceContent("updated").dispose();
  const local = await createLocalRunner(new URL("http://localhost:5000"), { signal });
  local.dispose();
  // @ts-expect-error The browser API does not mutate host files.
  rgd.run("delete", 38);
  return { exact, numbers, key, text, rendered, count };
}
void usage;
