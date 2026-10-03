// Advance only failed/quiet observations to the next already type-valid witness.
// Selection depends on observations, so this is exploration, not a holdout.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
const [selectionArg, browserArg, outputArg, historyArg] = process.argv.slice(2), selectionPath = resolve(selectionArg), browserPath = resolve(browserArg), output = resolve(outputArg);
assert(!existsSync(output)); const selection = read(selectionPath), browser = read(browserPath); assert(browser.finishedAt);
const history = historyArg ? read(resolve(historyArg)) : null;
const rows = [], exhausted = [];
for (const row of selection.rows) {
  const previous = history?.rows.find(r => r.package === row.package && r.export === row.export);
  if (previous?.confirmed) continue;
  const pair = browser.results.filter(r => r.packagePins[0].package === row.package && r.provenance.export === row.export); assert.equal(pair.length, 2);
  const unowned = pair.find(r => r.provenance.phase === 'unowned');
  if (pair.some(r => ![...r.observations, ...r.errors].some(o => o.label === 'invoke'))) { exhausted.push({ package: row.package, export: row.export, reason: 'Consumer module did not execute; observed loader failure' }); continue; }
  if (pair.some(r => r.consoleErrors.some(e => e.startsWith('Failed to load resource'))) && !pair.some(r => r.errors.some(e => e.label === 'invoke'))) {
    exhausted.push({ package: row.package, export: row.export, reason: 'Browser endpoint not supplied; no semantic inference from HTTP failure' }); continue;
  }
  const failures = pair.some(r => r.errors.length || r.pageErrors.length || r.harnessFailure || r.blockedRequests.length || r.publishedTypingErrors.length || r.consoleErrors.length);
  if (!failures && unowned.feedback.some(f => ['NO_OWNER_CLEANUP', 'NO_OWNER_EFFECT'].includes(f.code))) continue;
  const used = [...(row.previousArguments ?? []), row.arguments, ...(previous?.attempts.map(a => a.arguments) ?? [])], next = row.attempts.find(a => !a.typingErrors.length && !used.some(args => JSON.stringify(args) === JSON.stringify(a.arguments)));
  if (!next) { exhausted.push({ package: row.package, export: row.export, reason: 'No further published-type-valid witness' }); continue; }
  rows.push({ ...row, arguments: next.arguments, chosenSignature: next.signature, profile: next.profile, previousArguments: used,
    retryReason: failures ? 'runtime/domain failure' : 'ownership path not observed' });
}
const report = { ...selection, rows, refusals: [], exhausted, parentSelectionPath: selectionPath, parentSelectionSha256: hash(readFileSync(selectionPath)),
  ...(historyArg ? { historyPath: resolve(historyArg), historySha256: hash(readFileSync(resolve(historyArg))) } : {}),
  parentBrowserPath: browserPath, parentBrowserSha256: hash(readFileSync(browserPath)), summary: { selectedExports: rows.length, selectedPackages: new Set(rows.map(r => r.package)).size, exhausted: exhausted.length } };
writeFileSync(output, JSON.stringify(report, null, 2) + '\n'); console.log(JSON.stringify(report.summary));
