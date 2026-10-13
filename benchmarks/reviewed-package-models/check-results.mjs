// Reproject saved native observations with the final warning adapter. No
// unchanged source/binary analysis is repeated to check a reporting change.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { authenticateModel, hash, read } from "./catalog.mjs";
import { projectWarning, ts } from "./lower.mjs";
const here = dirname(fileURLToPath(import.meta.url));
const equivalent = process.env.REVIEWED_MODEL_EQUIVALENT_CATALOG ? read(resolve(process.env.REVIEWED_MODEL_EQUIVALENT_CATALOG)) : null;
for (const argument of process.argv.slice(2)) {
  const path = resolve(argument), report = read(path), out = dirname(path);
  assert.equal(report.authority, false); assert(report.summary && report.finishedAt, "Incomplete run");
  assert.equal(report.modelSha256, hash(readFileSync(report.catalogPath ?? process.env.REVIEWED_MODEL_REPLAY_CATALOG ?? join(here, "models.json"))), "Model inputs changed");
  assert.equal(report.summary.falsePositiveObservations, 0);
  for (const item of report.results) for (const observation of item.observations) {
    assert.equal(observation.publishedTypingErrors, 0); assert.equal(observation.analysisTwinTypingErrors, 0);
    const dir = join(out, item.id), stem = `${observation.host}-${observation.twin}`;
    const sourcePath = join(dir, `${stem}.tsx`), modeledPath = join(dir, `${stem}-modeled.tsx`);
    const source = ts.createSourceFile(sourcePath, readFileSync(sourcePath, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const lowered = read(join(dir, `${stem}-mapping.json`));
    if (observation.localCatalogSha256) {
      const local = read(join(dir, `${observation.host}-catalog.json`));
      assert.equal(observation.localCatalogSha256, hash(JSON.stringify(local, null, 2)));
      for (const model of local.models) authenticateModel(model, dir);
    }
    if (equivalent) for (const site of lowered.sites) {
      const model = equivalent.models.find(item => item.package === site.package);
      assert.deepEqual(model?.exports[site.export]?.[observation.host], site.behavior, `Exercised premise changed: ${item.id}/${stem}`);
      if (model) authenticateModel(model, dir);
    }
    assert.equal(readFileSync(modeledPath, "utf8"), lowered.text);
    const native = read(join(dir, `${stem}-modeled-output.json`));
    const warnings = native.findings.map(f => projectWarning(f, lowered, source, native.historicalAnalyzedPath ?? modeledPath)).filter(Boolean);
    const unique = [...new Map(warnings.map(w => [`${w.rule}:${w.location.startByte}`, w])).values()];
    assert.deepEqual(unique, observation.warnings, `Projection changed: ${item.id}/${stem}`);
    assert.equal(observation.reportsExpected, observation.expectedWarning ? unique.some(w => w.rule === item.rule) : unique.length === 0, `${item.id}/${stem}`);
  }
  console.log(`Validated ${report.summary.observations} saved observations: ${path}`);
}
