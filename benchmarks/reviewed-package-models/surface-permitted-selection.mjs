// Keep only the diagnosed pairs whose immediate callback caller is a package.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, validationArg, originsArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), validationPath = resolve(validationArg), originsPath = resolve(originsArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(selectionPath), validation = read(validationPath), origins = read(originsPath);
const proven = new Set(validation.results.filter(r => r.confirmedWritePair).map(r => r.package + '\0' + r.export));
const packageCallers = origins.results.filter(r => r.dependencyCallerObserved);
const keys = new Set(packageCallers.map(r => r.package + '\0' + r.export));
const rows = selection.rows.filter(r => proven.has(r.package + '\0' + r.export) && keys.has(r.package + '\0' + r.export));
assert(rows.length);
writeFileSync(output, JSON.stringify({ ...selection, rows, parentSelectionPath: selectionPath,
  parentSelectionSha256: hash(readFileSync(selectionPath)), historyPath: validationPath,
  historySha256: hash(readFileSync(validationPath)), callerEvidence: { path: originsPath, sha256: hash(readFileSync(originsPath)), results: packageCallers },
  selectionProducer: new URL('./surface-permitted-selection.mjs', import.meta.url).pathname,
  summary: { admittedExports: rows.length, admittedPackages: new Set(rows.map(r => r.package)).size } }, null, 2) + '\n');
console.log(JSON.stringify({ exports: rows.length }));
