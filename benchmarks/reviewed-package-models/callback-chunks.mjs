import assert from 'node:assert/strict';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, outputArg] = process.argv.slice(2), selectionPath = resolve(selectionArg), output = resolve(outputArg), selection = read(selectionPath);
assert(!existsSync(output)); mkdirSync(output, { recursive: true });
for (let i = 0; i < selection.rows.length; i += 80) writeFileSync(join(output, `chunk-${i / 80}.json`), JSON.stringify({ ...selection,
  rows: selection.rows.slice(i, i + 80), parentSelectionPath: selectionPath, parentSelectionSha256: hash(readFileSync(selectionPath)) }, null, 2) + '\n');
console.log(JSON.stringify({ exports: selection.rows.length, chunks: Math.ceil(selection.rows.length / 80) }));
