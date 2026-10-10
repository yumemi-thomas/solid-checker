// Reproject saved native observations without repeating native analysis.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { hash, read } from './catalog.mjs';
import { demandSpecializer } from './demand-models.mjs';
import { lower, ts } from './lower.mjs';
import { projectFamilyWarnings } from './family-project-warning.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [staticArg, outputArg] = process.argv.slice(2), input = resolve(staticArg), output = resolve(outputArg), statics = read(input);
assert(!existsSync(output)); assert(statics.finishedAt);
for (const pin of statics.inputs) assert.equal(hash(readFileSync(pin.path)), pin.sha256);
const browserPath = statics.inputs[1].path, rows = [];
for (const row of statics.results) {
  const result = { id: row.id, warnings: row.warnings, newlyProjected: [] }; rows.push(result);
  if (!row.modeled?.findings.some(f => f.kind === 'violation' && ['uncalled-accessor', 'prefer-for', 'resolve-in-tracked-scope',
    'until-in-tracked-scope', 'reactive-write-in-owned-scope', 'reactive-read-after-await', 'leaf-owner-forbidden-call'].includes(f.rule))) continue;
  const root = join(dirname(browserPath), row.id), path = join(root, 'src/main.tsx'), modeled = join(root, 'src/family-modeled.tsx');
  assert.equal(hash(readFileSync(path)), row.sourceSha256);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true, { customConditions: ['browser', 'development'] }), root).options;
  const program = ts.createProgram([path], options), source = program.getSourceFile(path);
  const lowered = lower(program, source, row.catalog, 'browser', undefined, demandSpecializer(row.catalog, root));
  assert.equal(hash(lowered.text), hash(readFileSync(modeled)), 'Reprojection must use the exact earlier analysis twin');
  result.warnings = projectFamilyWarnings(row.modeled.findings, lowered, source, modeled, program);
  const old = new Set(row.warnings.map(w => JSON.stringify([w.rule, w.location])));
  result.newlyProjected = result.warnings.filter(w => !old.has(JSON.stringify([w.rule, w.location])));
}
writeFileSync(output, JSON.stringify({ authority: false, certification: false, input: { path: input, sha256: hash(readFileSync(input)) },
  projectorSha256: hash(readFileSync(new URL('./family-project-warning.mjs', import.meta.url))), results: rows }, null, 2) + '\n');
console.log(JSON.stringify(rows.filter(r => r.newlyProjected.length).map(r => ({ id: r.id, rules: r.newlyProjected.map(w => w.rule) })), null, 2));
