import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import cases, { typeRejected } from "./holdout-cases.mjs";
import { hash, read } from "./catalog.mjs";
import { installedCatalog } from "./demand-models.mjs";
import { ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
const out = resolve(process.argv[2]); assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const retained = read("rust/target/primitives-checkpoint/run-browser.json");
const frozen = ["source-extractor.mjs", "lower.mjs", "demand-models.mjs"].map(name => ({ name, digest: hash(readFileSync(new URL(name, import.meta.url))) }));
const models = [], packages = [], rejected = [];
for (const name of new Set(cases.map(entry => entry.package))) {
  const row = retained.results.find(row => row.package === name); assert(row);
  const catalog = installedCatalog(row.retainedArtifacts.projectDir, [{ package: name, exports: cases.filter(entry => entry.package === name).map(entry => entry.export) }]);
  assert.equal(catalog.packages.filter(row => row.error).length, 0); models.push(...catalog.models); packages.push(...catalog.packages);
}
for (const entry of typeRejected) {
  const dir = join(out, entry.id); mkdirSync(dir);
  const row = retained.results.find(row => row.package === entry.package); symlinkSync(join(row.retainedArtifacts.projectDir, "node_modules"), join(dir, "node_modules"), "dir");
  const path = join(dir, "App.ts"); writeFileSync(path, entry.source);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions("v2", true, { customConditions: ["browser", "development"] }), dir).options;
  const diagnostics = ts.getPreEmitDiagnostics(ts.createProgram([path], options)).filter(d => d.category === ts.DiagnosticCategory.Error).map(d => ({ code: d.code, message: ts.flattenDiagnosticMessageText(d.messageText, "\n") }));
  assert(diagnostics.length > 0); rejected.push({ ...entry, diagnostics, excludedBeforeChecker: true });
}
const catalog = { format: "solid-checker-reviewed-model-experiment", version: 1, authority: false, basis: "source-extracted-assumption", runtime: "2.0.0-rc.9",
  extractorSha256: frozen[0].digest, models, packages };
writeFileSync(join(out, "catalog.json"), JSON.stringify(catalog, null, 2) + "\n");
// Selection includes exports with no premise, so missed coverage is counted.
writeFileSync(join(out, "selection.json"), JSON.stringify({ models: models.map(model => ({ package: model.package, version: model.version, exports: Object.fromEntries(model.requestedExports.map(name => [name, {}])) })) }, null, 2) + "\n");
writeFileSync(join(out, "study.json"), JSON.stringify({ authority: false, frozen, packagesPreviouslyInGenericCensus: true, independentHumanLabels: false, cases: cases.map(({ misuse, correct, ...rest }) => rest), typeRejected: rejected }, null, 2) + "\n");
console.log(JSON.stringify({ cases: cases.length, packages: models.length, typeRejected: rejected.length, frozen }));
