import assert from "node:assert/strict";
import { test } from "node:test";
import { existsSync, mkdirSync, mkdtempSync, realpathSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { nativeRuntimeRoots, read } from "./catalog.mjs";
import { argumentValue, demandSpecializer, installedCatalog } from "./demand-models.mjs";
import { lower, ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";

const retained = read(resolve("rust/target/primitives-checkpoint/run-browser.json"));
function consumer(name, code) {
  const row = retained.results.find(row => row.package === name);
  assert(row && existsSync(row.retainedArtifacts.projectDir), "Published install is required; tests must not skip");
  const dir = mkdtempSync(join(tmpdir(), "solid-demand-model-test-"));
  symlinkSync(join(row.retainedArtifacts.projectDir, "node_modules"), join(dir, "node_modules"), "dir");
  const path = join(dir, "App.tsx"); writeFileSync(path, code);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions("v2", true), dir).options;
  const program = ts.createProgram([path], options);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0, "Real published typings reject specimen");
  const catalog = installedCatalog(dir, [{ package: name, exports: [] }]);
  assert.equal(catalog.packages[0].error, null);
  const specialize = demandSpecializer(catalog, dir);
  return { program, source: program.getSourceFile(path), catalog, specialize, dir };
}

test("native runtime resolution follows pnpm transitive dependencies", () => {
  // Metadata-only resolver test; these manifests supply no semantic facts.
  const dir = mkdtempSync(join(tmpdir(), "solid-demand-pnpm-")), store = join(dir, ".pnpm", "solid", "node_modules");
  for (const name of ["solid-js", "@solidjs/signals", "@solidjs/web"]) {
    const root = join(store, name); mkdirSync(root, { recursive: true });
    writeFileSync(join(root, "package.json"), JSON.stringify({ name, version: "2.0.0-rc.9" }));
  }
  mkdirSync(join(dir, "node_modules")); symlinkSync(join(store, "solid-js"), join(dir, "node_modules", "solid-js"));
  assert.deepEqual(nativeRuntimeRoots(dir), ["solid-js", "@solidjs/signals", "@solidjs/web"].map(name => realpathSync(join(store, name))));
  const refused = installedCatalog(dir, [{ package: "missing-package", exports: ["factory"] }]);
  assert.equal(refused.models.length, 0); assert.match(refused.packages[0].error, /Installed package missing/);
});

test("closed arguments exclude getters, spreads, mutable identifiers and evaluated fields", () => {
  const value = text => argumentValue(ts.createSourceFile("App.ts", `const value = (${text});`, ts.ScriptTarget.Latest, true).statements[0].declarationList.declarations[0].initializer);
  assert.equal(value("() => 1").kind, "callable");
  assert.equal(value("{ enabled: true, delay: 10 }").kind, "object");
  for (const text of ["{ get enabled() { return true; } }", "{ enabled: true, ...input }", "delay", "{ enabled: foreign() }"])
    assert.equal(value(text).kind, "unknown", text);
});

test("installed timer models specialize argument branches and reuse only identical profiles", () => {
  const input = consumer("@solid-primitives/timer", `import { createTimer } from "@solid-primitives/timer";
createTimer(() => {}, 1000, setInterval); createTimer(() => {}, 1000, setInterval);
createTimer(() => {}, () => 1000, setInterval); const delay = 1000; createTimer(() => {}, delay, setInterval);`);
  const result = lower(input.program, input.source, input.catalog, "browser", undefined, input.specialize);
  assert.deepEqual(result.sites.map(site => site.behavior.owner), ["cleanup", "cleanup", "effect", undefined]);
  assert.equal(result.sites[3].applied, undefined); assert.equal(result.unsupported.length, 1);
  assert.deepEqual(input.specialize.stats, { extractions: 3, cacheHits: 1 });
  const changed = structuredClone(input.catalog); changed.extractorSha256 = "sha256:changed";
  assert.throws(() => demandSpecializer(changed, input.dir), /Extractor inputs changed/);
});

test("local wrapper summaries require exact stable symbols and a single synchronous return", () => {
  const prefix = `import { createMediaQuery } from "@solid-primitives/media";
function helper() { return createMediaQuery("x"); }`;
  const app = `export default function App() { const read = helper(); const value = read(); return <p>{String(value)}</p>; }`;
  const stable = consumer("@solid-primitives/media", prefix + app);
  const result = lower(stable.program, stable.source, stable.catalog, "browser", undefined, stable.specialize);
  assert.equal(result.sites.filter(site => site.wrapper && site.applied).length, 1);
  for (const code of [prefix + `const escaped = helper;` + app,
    prefix.replace(`return createMediaQuery("x");`, `if (Math.random()) return () => false; return createMediaQuery("x");`) + app]) {
    const input = consumer("@solid-primitives/media", code);
    const result = lower(input.program, input.source, input.catalog, "browser", undefined, input.specialize);
    assert.equal(result.sites.filter(site => site.wrapper).length, 0);
  }
});
