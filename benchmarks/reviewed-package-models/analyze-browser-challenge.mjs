import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { hash, read } from "./catalog.mjs";
import { installedCatalog, demandSpecializer } from "./demand-models.mjs";
import { lower, projectWarning, ts } from "./lower.mjs";
import { oracleCompilerOptions } from "../../scripts/tsc-oracle.mjs";
const reportPath = resolve(process.argv[2]), report = read(reportPath), observations = [];
const checker = resolve("rust/target/release/solid-checker-rust"), typefacts = resolve("bin/solid-typefacts");
const frozen = read("rust/target/reviewed-models-all-holdout-inputs/study.json").frozen;
for (const input of frozen) assert.equal(hash(readFileSync(new URL(input.name, import.meta.url))), input.digest);
for (const row of report.results.filter(row => row.provenance.syntheticBehavioralChallenge)) {
  const root = join(dirname(reportPath), row.id), path = join(root, "src/main.tsx");
  assert.equal(hash(readFileSync(path)), row.sourceSha256); assert.equal(row.publishedTypingErrors.length, 0);
  const catalog = installedCatalog(root, [{ package: "@solid-primitives/map", exports: ["ReactiveMap"] }]); assert.equal(catalog.packages[0].error, null);
  const options = oracleCompilerOptions("v2", true, { customConditions: ["browser", "development"] });
  const program = ts.createProgram([path], ts.convertCompilerOptionsFromJson(options, root).options);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const source = program.getSourceFile(path), lowered = lower(program, source, catalog, "browser", undefined, demandSpecializer(catalog, root));
  const modeled = join(root, "src/modeled.tsx"); writeFileSync(modeled, lowered.text);
  const item = { id: row.id, catalog, sites: lowered.sites, unsupported: lowered.unsupported, warnings: [], native: {} };
  for (const [variant, file] of [["baseline", path], ["modeled", modeled]]) {
    const project = join(root, `${variant}.json`); writeFileSync(project, JSON.stringify({ compilerOptions: options, files: [file] }));
    const result = spawnSync(checker, ["--format", "json", "--runtime-target", "browser", "--project", project], { env: { ...process.env, SOLID_TYPEFACTS_BIN: typefacts, SOLID_CHECKER_DAEMON: "0" }, encoding: "utf8", timeout: 30000 });
    assert([0, 1].includes(result.status), result.stderr); item.native[variant] = JSON.parse(result.stdout);
    if (variant === "modeled") item.warnings = item.native.modeled.findings.map(f => projectWarning(f, lowered, source, modeled)).filter(Boolean);
  }
  observations.push(item);
}
assert.equal(observations.length, 2);
writeFileSync(join(dirname(reportPath), "static-challenge.json"), JSON.stringify({ authority: false, frozen, checkerSha256: hash(readFileSync(checker)), typefactsSha256: hash(readFileSync(typefacts)), observations }, null, 2) + "\n");
console.log(JSON.stringify(observations.map(row => ({ id: row.id, sourcePremises: row.catalog.packages[0].observations.map(x => x.behavior), warnings: row.warnings.length, baseline: row.native.baseline.findings.map(f => ({ rule: f.rule, kind: f.kind })) }))));
