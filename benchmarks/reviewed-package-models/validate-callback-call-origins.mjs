// Refine the validated write observations; this does not create new findings.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { authenticateModel, hash, read } from './catalog.mjs';
import { callbackSites } from './callback-sites.mjs';
import { callbackCaller, callerRoots } from './callback-callers.mjs';
const [selectionArg, validationArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), validationPath = resolve(validationArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(selectionPath), validation = read(validationPath), catalog = read(selection.catalogPath);
const selected = new Map(selection.rows.map(r => [r.package + '\0' + r.export, r])), confirmed = new Map(validation.results
  .filter(r => r.confirmedWritePair).map(r => [r.package + '\0' + r.export, r]));
const roots = new Map(), results = [];
for (const study of validation.studies) for (const row of read(join(study.path, 'browser/results.json')).results) {
  if (row.provenance.role !== 'write') continue;
  const key = row.packagePins[0].package + '\0' + row.provenance.export;
  if (!confirmed.has(key) || !row.attributionEnabled) continue;
  const model = selected.get(key); assert(model);
  if (!roots.has(model.package)) {
    authenticateModel(catalog.models.find(m => m.package === model.package), model.project);
    roots.set(model.package, callerRoots(model.project, model.package));
  }
  const path = join(study.path, 'browser', row.id, 'src/main.tsx');
  assert.equal(hash(readFileSync(path)), row.sourceSha256);
  const sites = callbackSites(path), callers = row.feedback.filter(f => f.code === 'REACTIVE_WRITE_IN_OWNED_SCOPE')
    .map(f => callbackCaller(f, path, sites, roots.get(model.package))).filter(c => c.kind !== 'unmapped-write');
  assert(callers.length);
  results.push({ package: model.package, export: model.export, callers,
    dependencyCallerObserved: callers.some(c => c.kind === 'dependency'),
    solidCallerObserved: callers.some(c => c.kind === 'solid-runtime'),
    applicationOnly: callers.every(c => c.kind === 'application'),
    callerUnknown: callers.some(c => c.kind === 'unknown') });
}
assert.equal(results.length, confirmed.size);
const summary = { confirmedWriteApis: results.length, dependencyCallerApis: results.filter(r => r.dependencyCallerObserved).length,
  solidCallerApis: results.filter(r => r.solidCallerObserved).length,
  applicationOnlyApis: results.filter(r => r.applicationOnly).length,
  unknownCallerApis: results.filter(r => r.callerUnknown).length };
writeFileSync(output, JSON.stringify({ authority: false, certification: false,
  basis: 'immediate source-mapped callers of previously validated callback writes',
  inputs: [selectionPath, validationPath].map(path => ({ path, sha256: hash(readFileSync(path)) })), summary, results }, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
