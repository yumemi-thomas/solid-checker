// Explicit, package-specific format witnesses. This small relation table is
// reviewed experimental input, not an inferred contract or generic guarantee.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), selection = read(selectionPath), output = resolve(outputArg);
assert(!existsSync(output));
const producers = new Map([
  ['inflate', 'Package["deflateSync"](new Uint8Array([1, 2]))'],
  ['gunzip', 'Package["gzipSync"](new Uint8Array([1, 2]))'],
  ['unzlib', 'Package["zlibSync"](new Uint8Array([1, 2]))'],
  ['decompress', 'Package["gzipSync"](new Uint8Array([1, 2]))'],
  ['unzip', 'Package["zipSync"]({"sample": new Uint8Array([1, 2])})'],
]);
const rows = selection.rows.filter(r => producers.has(r.export)).map(row => ({ ...row,
  arguments: [producers.get(row.export), row.arguments[1]],
  deliveryChallenge: 'package producer output supplied to matching decoder' }));
assert.equal(rows.length, producers.size);
writeFileSync(output, JSON.stringify({ ...selection, rows, parentSelectionPath: selectionPath,
  parentSelectionSha256: hash(readFileSync(selectionPath)), selectionProducer: new URL('./binary-roundtrip-selection.mjs', import.meta.url).pathname,
  summary: { admittedExports: rows.length, admittedPackages: 1 } }, null, 2) + '\n');
console.log(JSON.stringify({ exports: rows.length }));
