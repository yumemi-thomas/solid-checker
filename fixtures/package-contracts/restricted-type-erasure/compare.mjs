// Regression experiment only. No certification request, receipt or loader hook.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { pathToFileURL } from "node:url";

const ts = (await import(pathToFileURL(process.argv[2]).href)).default;
assert.equal(ts.version, "5.9.3", "review the comparator when its pin changes");
const source = readFileSync(new URL("reflect.ts", import.meta.url), "utf8");
const stripped = stripTypeScriptTypes(source, { mode: "strip" });
const compiled = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext }
}).outputText;
assert.notEqual(stripped, compiled);
async function observe(code) {
  delete globalThis.__profileContradiction;
  const module = await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
  assert.equal(module.subject(), 1);
  return globalThis.__profileContradiction === true;
}
assert.equal(await observe(stripped), false);
assert.equal(await observe(compiled), true);
// A contradiction present in the stripped bytes remains observable. This
// checks the experiment, not a production derived-byte certification gate.
assert.equal(await observe(stripTypeScriptTypes(
  "export function subject(value: number = 1) { globalThis.__profileContradiction = true; return value; }",
  { mode: "strip" }
)), true);
assert.throws(() => stripTypeScriptTypes("export enum Mode { A }", { mode: "strip" }));
assert.throws(() => stripTypeScriptTypes("export const view = <div/>;", { mode: "strip" }));
await assert.rejects(import("./extensionless.mjs"), { code: "ERR_MODULE_NOT_FOUND" });
process.stdout.write("strip=false;typescript=true;derived-contradiction=true;enum=refused;tsx=refused\n");
