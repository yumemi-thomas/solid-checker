import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, validationArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), validationPath = resolve(validationArg), output = resolve(outputArg);
assert(!existsSync(output));
const selection = read(selectionPath), validation = read(validationPath);
const keys = new Set(validation.results.filter(r => r.confirmedWritePair).map(r => r.package + '\0' + r.export));
const rows = selection.rows.filter(r => keys.has(r.package + '\0' + r.export)); assert(rows.length);
writeFileSync(output, JSON.stringify({ ...selection, rows, historyPath: validationPath,
  historySha256: hash(readFileSync(validationPath)), summary: { admittedExports: rows.length,
    admittedPackages: new Set(rows.map(r => r.package)).size } }, null, 2) + '\n');
console.log(JSON.stringify({ exports: rows.length }));
