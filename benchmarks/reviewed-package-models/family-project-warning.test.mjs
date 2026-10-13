import assert from 'node:assert/strict';
import { test } from 'node:test';
import { existsSync, mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { read } from './catalog.mjs';
import { installedCatalog, demandSpecializer } from './demand-models.mjs';
import { lower, ts } from './lower.mjs';
import { projectFamilyWarning, projectFamilyWarnings } from './family-project-warning.mjs';
import { familyRequests } from './family-imports.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const retained = read(resolve('rust/target/primitives-checkpoint/run-browser.json'));
function consumer(packageName, code) {
  const installed = retained.results.find(row => row.package === packageName)?.retainedArtifacts.projectDir;
  assert(installed && existsSync(installed), 'Real published install is required');
  const dir = mkdtempSync(join(tmpdir(), 'solid-family-origin-')); symlinkSync(join(installed, 'node_modules'), join(dir, 'node_modules'), 'dir');
  const path = join(dir, 'App.tsx'); writeFileSync(path, code);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), dir).options, program = ts.createProgram([path], options);
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error).length, 0);
  const source = program.getSourceFile(path), imports = familyRequests(program, source, [packageName]), catalog = installedCatalog(dir, imports.requests);
  const lowered = lower(program, source, catalog, 'browser', undefined, demandSpecializer(catalog, dir));
  return { program, source, imports, lowered, path, modeled: join(dir, 'modeled.tsx') };
}
function finding(input, rule, start, end) {
  const segment = input.lowered.segments.find(s => s.copied && s.originalStart <= start && end <= s.originalStart + s.end - s.start);
  assert(segment, 'Operation must retain an exact copied span');
  const offset = original => segment.start + original - segment.originalStart;
  return { kind: 'violation', rule, id: 'native-rule', message: 'saved native operation', primaryLocation: { path: input.modeled,
    startByte: Buffer.byteLength(input.lowered.text.slice(0, offset(start))), endByte: Buffer.byteLength(input.lowered.text.slice(0, offset(end))) } };
}
const prefix = `import { createReducer } from '@solid-primitives/memo'; const [value] = createReducer((s: number) => s, 1);`;
test('exact accessor symbol maps a native operand through Unicode source', () => {
  const input = consumer('@solid-primitives/memo', `// 日本語\n${prefix}\nconst text = \`count:${'${value}'}\`;`);
  const start = input.source.text.lastIndexOf('value'), f = finding(input, 'uncalled-accessor', start, start + 5);
  const warning = projectFamilyWarning(f, input.lowered, input.source, input.modeled, input.program);
  assert.equal(warning.package, '@solid-primitives/memo'); assert.equal(warning.projectedPremise, 'returned-accessor');
  assert.equal(warning.location.startByte, Buffer.byteLength(input.source.text.slice(0, start))); assert.equal(warning.location.line, 3);
  assert.equal(warning.certification, false);
  assert.equal(projectFamilyWarning({ ...f, kind: 'uncertifiable' }, input.lowered, input.source, input.modeled, input.program), null);
});
test('a shadowed operand cannot inherit another binding premise', () => {
  const input = consumer('@solid-primitives/memo', `${prefix}\nfunction other(value: () => number) { return \`count:${'${value}'}\`; }`);
  const start = input.source.text.lastIndexOf('value'), f = finding(input, 'uncalled-accessor', start, start + 5);
  assert.equal(projectFamilyWarning(f, input.lowered, input.source, input.modeled, input.program), null);
});
test('ambiguous premises and absent source facts remain unprojected', () => {
  const input = consumer('@solid-primitives/memo', `${prefix}\nconst text = \`count:${'${value}'}\`;`);
  const start = input.source.text.lastIndexOf('value'), f = finding(input, 'uncalled-accessor', start, start + 5);
  for (const sites of [[], [...input.lowered.sites, ...input.lowered.sites]])
    assert.equal(projectFamilyWarning(f, { ...input.lowered, sites }, input.source, input.modeled, input.program), null);
});
test('leaf cleanup cannot also be described as owner absence at the same operation', () => {
  const input = consumer('@solid-primitives/utils', `import { createTrackedEffect } from 'solid-js'; import { createMicrotask } from '@solid-primitives/utils';
createTrackedEffect(() => { const run = createMicrotask(() => {}); run(); });`);
  const site = input.lowered.sites[0], segment = input.lowered.segments.find(s => !s.copied && s.originalStart === site.start);
  const f = { kind: 'violation', message: 'cleanup', primaryLocation: { path: input.modeled, startByte: Buffer.byteLength(input.lowered.text.slice(0, segment.start)) } };
  const warnings = projectFamilyWarnings([{ ...f, rule: 'leaf-owner-forbidden-call' }, { ...f, rule: 'missing-owner' }], input.lowered, input.source, input.modeled, input.program);
  assert.deepEqual(warnings.map(w => w.rule), ['leaf-owner-forbidden-call']);
  assert.deepEqual(projectFamilyWarnings([{ ...f, rule: 'missing-owner' }], input.lowered, input.source, input.modeled, input.program).map(w => w.rule), ['missing-owner']);
});
test('namespace requests require exact import symbols and reject computed dispatch', () => {
  const input = consumer('@solid-primitives/timer', `import * as Timer from '@solid-primitives/timer';
const counter = Timer.createIntervalCounter(100000); const key = 'createIntervalCounter'; Timer[key](100000);
function shadow(Timer: { createIntervalCounter(n: number): () => number }) { return Timer.createIntervalCounter(1); }`);
  assert.deepEqual(input.imports.requests, [{ package: '@solid-primitives/timer', exports: ['createIntervalCounter'] }]);
  assert.equal(input.imports.gaps.length, 1);
});
