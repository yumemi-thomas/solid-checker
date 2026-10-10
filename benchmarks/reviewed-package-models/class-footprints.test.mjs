import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { test } from 'node:test';
import { ClassFootprints, classSnapshotFlows } from './class-footprints.mjs';
import { read } from './catalog.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';

function observe(text, member) {
  const root = mkdtempSync(join(tmpdir(), 'class-footprint-')), path = join(root, 'index.js');
  writeFileSync(path, text); const engine = new ClassFootprints();
  const owner = engine.exported(path, 'Example');
  return engine.footprint(engine.method(owner, member));
}
test('private field calls resolve the exact local class and aliased core symbol', () => {
  const result = observe(`import { getObserver as observer } from 'solid-js';
class Track { run() { if (!observer()) return; } }
export class Example { #track = new Track(); read() { this.#track.run(); return 1; } }`, 'read');
  assert.equal(result.premises.length, 1);
});
test('shadowed core names and public member dispatch remain open', () => {
  for (const text of [
    `import { getObserver } from 'solid-js'; export class Example { read() { function getObserver() { return null; } return getObserver(); } }`,
    `import { getObserver } from 'solid-js'; class Track { run() { getObserver(); } } export class Example { track = new Track(); read() { this.track.run(); } }`,
    `import { getObserver } from 'solid-js'; class Track { run() { getObserver(); } } export class Example { #track = new Track(); replace() { this.#track = null; } read() { this.#track.run(); } }`,
    `import { getObserver } from 'solid-js'; class Track { constructor() { this.run = () => null; } run() { getObserver(); } } export class Example { #track = new Track(); read() { this.#track.run(); } }`,
  ]) assert.equal(observe(text, 'read').premises.length, 0);
});
test('generator creation and delegation do not execute the deferred footprint', () => {
  const text = `import { getObserver } from 'solid-js'; export class Example {
    *values() { getObserver(); yield 1; } keys() { return this.values(); } get size() { getObserver(); return 1; }
  }`;
  assert.equal(observe(text, 'values').premises.length, 0);
  assert.equal(observe(text, 'keys').premises.length, 0);
  assert.equal(observe(text, 'size').premises.length, 1);
});
test('stored and unknown-timing callbacks do not become immediate reads', () => {
  assert.equal(observe(`import { getObserver } from 'solid-js'; export class Example {
    read() { const later = () => getObserver(); unknown(later); return later; }
  }`, 'read').premises.length, 0);
});
test('an observer footprint alone does not prove that the returned value is reactive', () => {
  const result = observe(`import { getObserver } from 'solid-js'; export class Example {
    read() { if (getObserver()) console.log('observed'); return 42; }
  }`, 'read');
  // This positive syntactic observation is intentionally still informational.
  assert.equal(result.premises.length, 1);
});
test('published member identity survives transparent callee wrappers and type-only references', () => {
  const row = read(resolve('rust/target/primitives-checkpoint/run-browser.json')).results.find(row => row.package === '@solid-primitives/map');
  assert(row && existsSync(row.retainedArtifacts.projectDir), 'Published install is required; do not skip');
  const root = mkdtempSync(join(tmpdir(), 'class-typed-wrapper-'));
  symlinkSync(join(row.retainedArtifacts.projectDir, 'node_modules'), join(root, 'node_modules'), 'dir');
  const path = join(root, 'App.tsx');
  writeFileSync(path, `import { ReactiveMap } from '@solid-primitives/map';
    export function App() { const map = new ReactiveMap<string, number>([['key', 0]]);
      const value = ((map.get) as typeof map.get)('key'); return <p>{String(value)}</p>; }`);
  const options = ts.convertCompilerOptionsFromJson(oracleCompilerOptions('v2', true), root).options;
  const program = ts.createProgram([path], options);
  assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d => d.category === ts.DiagnosticCategory.Error), []);
  const result = classSnapshotFlows(program, program.getSourceFile(path));
  assert.equal(result.candidates.length, 1); assert.equal(result.refused.length, 0);
});
