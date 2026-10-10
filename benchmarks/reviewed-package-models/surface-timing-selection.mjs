// Counterexamples to any blanket callback-write prohibition for these methods.
// All options still pass the published-types oracle in the browser driver.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, outputArg] = process.argv.slice(2);
const selectionPath = resolve(selectionArg), selection = read(selectionPath), output = resolve(outputArg);
assert(!existsSync(output));
const rows = selection.rows.filter(r => r.package === 'lodash' && ['debounce', 'throttle'].includes(r.export)).map(row => {
  const args = [...row.arguments];
  if (args[2]) { assert(args[2].includes('"leading": true')); args[2] = args[2].replace('"leading": true', '"leading": false'); }
  else { assert.equal(args.length, 2); args.push('{"leading": false, "trailing": true}'); }
  return { ...row, arguments: args,
    timingChallenge: 'same owned construction and default signal, trailing delivery only' };
});
assert.equal(rows.length, 2);
writeFileSync(output, JSON.stringify({ ...selection, rows, parentSelectionPath: selectionPath,
  parentSelectionSha256: hash(readFileSync(selectionPath)), selectionProducer: new URL('./surface-timing-selection.mjs', import.meta.url).pathname,
  summary: { admittedExports: rows.length, admittedPackages: 1 } }, null, 2) + '\n');
console.log(JSON.stringify({ exports: rows.length }));
