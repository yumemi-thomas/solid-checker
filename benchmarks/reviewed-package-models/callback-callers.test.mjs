import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdirSync, mkdtempSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { callbackSites } from './callback-sites.mjs';
import { callbackCaller } from './callback-callers.mjs';
test('caller classification preserves a missing immediate frame instead of skipping it', () => {
  const root = mkdtempSync(join(tmpdir(), 'callback-caller-')), consumer = join(root, 'consumer.ts');
  writeFileSync(consumer, `import {createSignal} from 'solid-js'; const [read, write] = createSignal(1);
const callback = () => { h.samples.push({ id: 0 }); write(2); }; callback();`);
  const sites = callbackSites(consumer), site = sites.writes[0], position = sites.source.getLineAndCharacterOfPosition(site.start);
  const location = { path: consumer, line: position.line + 1, column: position.character + 1 };
  const application = { path: consumer, line: 2, column: 70 };
  assert.equal(callbackCaller({ originalLocation: location, originalFrames: [location, application] }, consumer, sites, new Map()).kind, 'application');
  const gap = callbackCaller({ originalLocation: location, originalFrames: [location, null, application] }, consumer, sites, new Map());
  assert.equal(gap.kind, 'unknown'); assert.match(gap.reason, /Immediate/);
  const dependency = join(root, 'package', 'index.js');
  const missing = callbackCaller({ originalLocation: location, originalFrames: [location, { path: dependency }] }, consumer, sites,
    new Map([[join(root, 'package'), { package: 'example', version: '1.0.0' }]]));
  assert.equal(missing.kind, 'unknown');
});
test('an exact served file can identify a caller, while optimized bundle names stay open', () => {
  const root = mkdtempSync(join(tmpdir(), 'callback-served-')), consumer = join(root, 'consumer.ts'), packageDir = join(root, 'package');
  mkdirSync(packageDir); const dependency = join(packageDir, 'index.js'); writeFileSync(dependency, 'export const call = fn => fn();');
  writeFileSync(consumer, `import {createSignal} from 'solid-js'; const [, write] = createSignal(1);
const callback = () => { h.samples.push({ id: 0 }); write(2); };`);
  const sites = callbackSites(consumer), p = sites.source.getLineAndCharacterOfPosition(sites.writes[0].start);
  const location = { path: consumer, line: p.line + 1, column: p.character + 1 };
  const graph = new Map([[realpathSync(packageDir), { package: 'example', version: '1.0.0' }]]);
  const diagnostic = { originalLocation: location, originalFrames: [location, null],
    frames: [{ path: 'http://localhost/src/main.tsx' }, { path: 'http://localhost/@fs' + dependency + '?v=1' }] };
  const result = callbackCaller(diagnostic, consumer, sites, graph);
  assert.equal(result.kind, 'dependency'); assert.equal(result.package, 'example'); assert.equal(result.callerFileBasis, 'served-input-module');
  assert.equal(result.caller.line, undefined);
  diagnostic.frames[1].path = 'http://elsewhere/@fs' + dependency;
  assert.equal(callbackCaller(diagnostic, consumer, sites, graph).kind, 'unknown');
  diagnostic.frames[1].path = 'http://localhost/.vite-cache/deps/example.js?v=1';
  assert.equal(callbackCaller(diagnostic, consumer, sites, graph).kind, 'unknown');
});
