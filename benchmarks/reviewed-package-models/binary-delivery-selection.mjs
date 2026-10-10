// A finite delivery challenge for the retained compression installation.
// Published FlateCallback accepts an error and data; its returned function
// cancels delivery. The browser oracle checks each adapted call again.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { callbackArguments } from './callback-paths.mjs';
const [selectionArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), output = resolve(outputArg), selection = read(selectionPath);
assert(!existsSync(output));
const rows = selection.rows.filter(row => row.package === 'fflate').map(row => {
  assert.equal(row.consume, 'self'); assert.equal(row.arguments.at(-1), '() => (undefined)');
  const args = [row.arguments[0], '(err, data) => (h.values.callbackError = err?.message ?? null, h.values.callbackSucceeded = err === null, h.values.callbackDataPresent = data !== undefined, undefined)'];
  const source = ts.createSourceFile('delivery.ts', `candidate(${args.join(', ')});`, ts.ScriptTarget.Latest, true);
  return { ...row, arguments: args, callbacks: callbackArguments(source.statements[0].expression.arguments).callbacks,
    consume: null, deliveryChallenge: 'await worker callback before invoking returned terminator' };
});
assert(rows.length);
writeFileSync(output, JSON.stringify({ ...selection, rows, parentSelectionPath: selectionPath,
  parentSelectionSha256: hash(readFileSync(selectionPath)), selectionProducer: new URL('./binary-delivery-selection.mjs', import.meta.url).pathname,
  summary: { admittedExports: rows.length, admittedPackages: 1 } }, null, 2) + '\n');
console.log(JSON.stringify({ exports: rows.length }));
