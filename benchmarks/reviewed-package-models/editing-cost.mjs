// Measure the existing model path, including bytes authentication. Mutation
// tests copy installed artifacts; they never alter retained package inputs.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { authenticateModel, closurePins, hash, packageRoot, read } from "./catalog.mjs";
import { installedCatalog, demandSpecializer } from "./demand-models.mjs";
import { lower, ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
const out = resolve(process.argv[2]); assert(!existsSync(out)); mkdirSync(out, { recursive: true });
const retained = read("rust/target/primitives-checkpoint/run-browser.json");
const row = retained.results.find(row => row.package === "@solid-primitives/timer"), root = join(out, "consumer"); mkdirSync(root);
symlinkSync(join(row.retainedArtifacts.projectDir, "node_modules"), join(root, "node_modules"), "dir");
const report = { authority: false, node: process.version, typescript: ts.version, samples: [], invalidation: [], nativeSamples: [], limitations: ["one small consumer", "warm filesystem cache", "no language server integration", "native checker launched separately on each measured edit", "artifact changes are simulated, not published releases"] };
const timed = fn => { const start = performance.now(), result = fn(); return { result, durationMs: performance.now() - start }; };
const catalog = timed(() => installedCatalog(root, [{ package: row.package, exports: [] }]));
assert.equal(catalog.result.packages[0].error, null); report.catalogMs = catalog.durationMs;
const specializer = timed(() => demandSpecializer(catalog.result, root)); report.specializerMs = specializer.durationMs;
const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions("v2", true, { customConditions: ["browser", "development"] }), root).options;
const path = join(root, "App.tsx"), compilerHost = ts.createCompilerHost(options), originalGetSourceFile = compilerHost.getSourceFile.bind(compilerHost), files = new Map();
const stats = { sourceParses: 0, sourceCacheHits: 0 };
compilerHost.getSourceFile = (path, languageVersion, onError, shouldCreateNewSourceFile) => {
  const text = ts.sys.readFile(path), prior = files.get(path);
  if (prior?.text === text) { stats.sourceCacheHits++; return prior.source; }
  const source = originalGetSourceFile(path, languageVersion, onError, shouldCreateNewSourceFile); stats.sourceParses++;
  if (source) files.set(path, { text, source }); return source;
};
let program;
for (let index = 0; index < 12; index++) {
  // Ten body-only edits preserve the factory's argument profile. The final
  // edit changes a numeric delay to an inline accessor and needs one new model.
  const delay = index === 11 ? "() => 10" : "10";
  writeFileSync(path, `import { createTimer } from '@solid-primitives/timer'; export default function App() { createTimer(() => {}, ${delay}, setInterval); return <p>edit ${index}</p>; }`);
  const before = { ...specializer.result.stats }, parseBefore = stats.sourceParses;
  const type = timed(() => { program = ts.createProgram({ rootNames: [path], options, host: compilerHost, oldProgram: program }); return ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error); });
  assert.equal(type.result.length, 0);
  const prepare = timed(() => lower(program, program.getSourceFile(path), catalog.result, "browser", undefined, specializer.result));
  report.samples.push({ index, delayKind: index === 11 ? "callable" : "literal", typecheckMs: type.durationMs, modelAndLowerMs: prepare.durationMs,
    totalPreparationMs: type.durationMs + prepare.durationMs, newParses: stats.sourceParses - parseBefore, newExtractions: specializer.result.stats.extractions - before.extractions, cacheHits: specializer.result.stats.cacheHits - before.cacheHits });
  if ([0, 5, 11].includes(index)) {
    const generated = join(root, `edit-${index}-modeled.tsx`), project = join(root, `edit-${index}.json`);
    writeFileSync(generated, prepare.result.text); writeFileSync(project, JSON.stringify({ compilerOptions: oracleCompilerOptions("v2", true, { customConditions: ["browser", "development"] }), files: [generated] }));
    const checker = resolve("rust/target/release/solid-checker-rust"), typefacts = resolve("bin/solid-typefacts");
    const native = timed(() => spawnSync(checker, ["--format", "json", "--runtime-target", "browser", "--project", project], { env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_DAEMON: "0" }, encoding: "utf8", timeout: 30000 }));
    assert([0, 1].includes(native.result.status), native.result.stderr); const output = JSON.parse(native.result.stdout);
    writeFileSync(join(root, `edit-${index}-output.json`), JSON.stringify(output, null, 2) + "\n");
    report.nativeSamples.push({ index, durationMs: native.durationMs, endToEndMs: native.durationMs + type.durationMs + prepare.durationMs, checkerSha256: hash(readFileSync(checker)), typefactsSha256: hash(readFileSync(typefacts)) });
  }
}
assert.deepEqual(report.samples.map(row => row.newExtractions), [1, ...Array(10).fill(0), 1]);
report.sourceStats = stats; report.specializationStats = specializer.result.stats;
const mutations = join(out, "mutations"), copy = join(mutations, "node_modules/@solid-primitives/timer"); mkdirSync(dirname(copy), { recursive: true });
const original = packageRoot(root, row.package);
cpSync(original, copy, { recursive: true, filter: path => !path.includes('/node_modules/', original.length) });
// Preserve exact dependency resolution for the relocated copy.
mkdirSync(join(copy, "node_modules"));
const manifest = read(join(copy, "package.json"));
for (const name of Object.keys({ ...manifest.dependencies, ...manifest.peerDependencies })) {
  const link = join(copy, "node_modules", name); mkdirSync(dirname(link), { recursive: true }); symlinkSync(packageRoot(original, name), link, "dir");
}
symlinkSync(packageRoot(root, "solid-js"), join(mutations, "node_modules/solid-js"), "dir");
const model = structuredClone(catalog.result.models[0]); assert.deepEqual(closurePins(copy), model.pins); authenticateModel(model, mutations);
const sourcePath = join(copy, "dist/index.js"), source = readFileSync(sourcePath, "utf8"), packagePath = join(copy, "package.json"), packageText = readFileSync(packagePath, "utf8");
const runMutation = (label, edit, expectedRefused) => {
  edit(); const check = timed(() => { try { authenticateModel(model, mutations); return { refused: false }; } catch (error) { return { refused: true, message: error.message }; } });
  assert.equal(check.result.refused, expectedRefused); report.invalidation.push({ label, ...check.result, durationMs: check.durationMs });
};
runMutation("unrelated consumer edit", () => writeFileSync(join(mutations, "unrelated.ts"), "export const value = 1;"), false);
runMutation("same version, changed runtime bytes", () => writeFileSync(sourcePath, source + "\n// simulated artifact mutation\n"), true);
writeFileSync(sourcePath, source);
runMutation("same version, changed public typings", () => writeFileSync(join(copy, "dist/index.d.ts"), readFileSync(join(copy, "dist/index.d.ts"), "utf8") + "\n// simulated declaration mutation\n"), true);
// Restore only the copied declaration before the next, separately measured input.
cpSync(join(original, "dist/index.d.ts"), join(copy, "dist/index.d.ts"));
runMutation("changed package version", () => writeFileSync(packagePath, JSON.stringify({ ...manifest, version: "0.0.0-experiment" })), true);
writeFileSync(packagePath, packageText);
const refresh = timed(() => installedCatalog(mutations, [{ package: row.package, exports: [] }]));
assert.equal(refresh.result.packages[0].error, null); report.refreshMs = refresh.durationMs;
assert.deepEqual(refresh.result.models[0].pins, model.pins);
const summary = values => { const ordered = values.toSorted((a, b) => a - b); return { medianMs: ordered[Math.floor(ordered.length / 2)], p95Ms: ordered[Math.ceil(ordered.length * .95) - 1], samples: values.length }; };
report.warmPreparation = summary(report.samples.slice(1, 11).map(row => row.totalPreparationMs));
report.warmLower = summary(report.samples.slice(1, 11).map(row => row.modelAndLowerMs));
writeFileSync(join(out, "results.json"), JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ catalogMs: report.catalogMs, specializerMs: report.specializerMs, warmPreparation: report.warmPreparation, warmLower: report.warmLower, native: report.nativeSamples, specializationStats: report.specializationStats, invalidation: report.invalidation }, null, 2));
