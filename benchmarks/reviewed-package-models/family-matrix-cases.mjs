// Authored consumers and expectations are separate from every detector.
// The matrix deliberately retains unknown dispatch and a TypeScript exclusion.
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname);
const primitive = read(resolve(repo, 'rust/target/primitives-checkpoint/run-browser.json'));
const foreign = read(resolve(repo, 'rust/target/cross-package-roots/run.json'));
const cases = [];
const prelude = `import { createSignal, createRoot, createMemo, createTrackedEffect, onCleanup, Loading, Show, resolve, until, action, flush } from 'solid-js';
import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function install(name) {
  const row = [...primitive.results, ...foreign.results].find(row => row.package === name);
  if (!row) throw new Error('No retained installation for ' + name);
  return relative(resolve(repo, 'rust/target/app-import-metric/apps'), row.retainedArtifacts.projectDir);
}
const finish = async page => page.evaluate(() => { const h = globalThis.__experiment; h.dispose?.(); h.disposals++; });
const settle = async page => { await page.waitForTimeout(25); await finish(page); };
function pair(id, name, imports, setup, jsx, flow = settle, expected = {}) {
  for (const bad of [true, false]) cases.push({ id: `family-${id}-${bad ? 'target' : 'control'}`, package: name, app: install(name),
    source: `${prelude}\n${imports}\nfunction App() { ${setup(bad)} return ${jsx(bad)}; }
h.attempt('mount', () => { h.dispose = render(() => <App />, document.getElementById('root')!); });`, flow,
    provenance: { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, ...expected } });
}
const reducer = `import { createReducer } from '@solid-primitives/memo';`;
const update = async page => {
  await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('update', h.update); });
  await page.waitForTimeout(25); await page.evaluate(() => { const h = globalThis.__experiment;
    h.values.behavior = { desired: '2', actual: document.getElementById('value')?.textContent ?? null }; }); await finish(page);
};
// flush is a module binding, so expose the actual update operation to the page.
const reducerSetup = `const [value, dispatch] = createReducer((state: number, increment: number) => state + increment, 1); h.update = () => { dispatch(1); flush(); };`;
pair('reducer-snapshot', '@solid-primitives/memo', reducer,
  bad => `${reducerSetup} ${bad ? 'const frozen = value();' : ''}`, bad => `<p id='value'>{${bad ? 'frozen' : 'value()'}}</p>`, update,
  { family: 'silent-staleness', rules: ['strict-read-untracked'], behaviorExpectation: 'render follows reducer dispatch' });
pair('uncalled-accessor', '@solid-primitives/memo', reducer,
  () => reducerSetup, bad => `<p id='value'>{${bad ? '`count:${value}`' : '`count:${value()}`'}}</p>`,
  async page => { await page.evaluate(() => { const h = globalThis.__experiment;
    h.values.behavior = { desired: 'count:1', actual: document.getElementById('value')?.textContent ?? null }; }); await finish(page); },
  { family: 'accessor-value', rules: ['uncalled-accessor'], behaviorExpectation: 'render reducer value, not its function text' });
const storeImports = `import { createStaticStore } from '@solid-primitives/static-store';`;
pair('store-snapshot', '@solid-primitives/static-store', storeImports,
  bad => `const [state, setState] = createStaticStore({ count: 1 }); h.update = () => { setState('count', 2); flush(); }; ${bad ? 'const frozen = state.count;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'state.count'}}</p>`, update,
  { family: 'silent-staleness', rules: ['strict-read-untracked'], behaviorExpectation: 'render follows store setter' });
pair('store-write', '@solid-primitives/static-store', storeImports,
  bad => `const [state, setState] = createStaticStore({ count: 1 }); h.update = () => { ${bad ? 'state.count = 2' : "setState('count', 2)"}; flush(); };`,
  () => `<p id='value'>{state.count}</p>`, update,
  { family: 'runtime-property', rules: ['no-direct-mutation'], behaviorExpectation: 'use the actual setter to update the value' });
pair('jsx-content', '@solid-primitives/memo', reducer,
  () => reducerSetup, bad => bad ? `<div id='value' innerHTML={'<b>other</b>'}>{value()}</div>` : `<div id='value'>{value()}</div>`, settle,
  { family: 'compiler-jsx', rules: ['jsx-no-duplicate-props'] });
pair('list-rendering', '@solid-primitives/memo', reducer,
  () => `const [items] = createReducer((state: number[]) => state, [1, 2]);`,
  bad => bad ? `<div>{items().map(item => <span>{item}</span>)}</div>` : `<div><For each={items()}>{item => <span>{item}</span>}</For></div>`, settle,
  { family: 'rendering-policy', rules: ['prefer-for'], preference: true });
// The correct list form has a real import, not a fixture declaration.
cases.filter(row => row.provenance.pair === 'list-rendering').forEach(row => row.source = `import { For } from 'solid-js';\n` + row.source);
const lazy = `import { createLazyMemo } from '@solid-primitives/memo';`;
pair('lazy-write', '@solid-primitives/memo', lazy,
  bad => `const [count, setCount] = createSignal(1); const value = createLazyMemo(() => { ${bad ? 'setCount(2);' : ''} return count(); });`,
  () => `<p>{String(value())}</p>`, settle,
  { family: 'owned-write', rules: ['reactive-write-in-owned-scope'], codes: ['REACTIVE_WRITE_IN_OWNED_SCOPE'] });
for (const api of ['resolve', 'until']) pair(api + '-tracked', '@solid-primitives/memo', lazy,
  bad => `const [count] = createSignal(1); const value = createLazyMemo(() => ${bad ? `${api}(() => count())` : 'count()'});`,
  () => `<p>{String(value())}</p>`, settle,
  { family: 'tracked-operation', rules: [api + '-in-tracked-scope'], exceptionExpected: true });
const microtask = `import { createMicrotask } from '@solid-primitives/utils';`;
pair('leaf-cleanup', '@solid-primitives/utils', microtask,
  bad => `${bad ? '' : 'const run = createMicrotask(() => {});'} createTrackedEffect(() => { ${bad ? 'const run = createMicrotask(() => {});' : ''} run(); });`,
  () => `<p>leaf</p>`, settle,
  { family: 'leaf-owner', rules: ['leaf-owner-forbidden-call'], codes: ['CLEANUP_IN_FORBIDDEN_SCOPE'] });
for (const bad of [true, false]) cases.push({ id: `family-owner-${bad ? 'target' : 'control'}`, package: '@solid-primitives/utils', app: install('@solid-primitives/utils'),
  source: `${prelude}\n${microtask}\n${bad ? "h.attempt('invoke', () => createMicrotask(() => {})); h.dispose = () => {};" : "h.dispose = createRoot(dispose => { createMicrotask(() => {}); return dispose; });"}`,
  flow: settle, provenance: { pair: 'owner', family: 'missing-owner', role: bad ? 'target' : 'control', expectedIssue: bad,
    rules: ['missing-owner'], codes: ['NO_OWNER_CLEANUP'] } });
pair('rxjs-callback', 'rxjs', `import { of, map } from 'rxjs';`,
  bad => `const [read, set] = createSignal(1); const run = () => of(1).pipe(map(() => { set(2); return read(); })).subscribe();
    ${bad ? 'run();' : 'h.run = run;'}`,
  () => `<p>callback</p>`, async page => { await page.waitForTimeout(25); await page.evaluate(() => globalThis.__experiment.run?.()); await page.waitForTimeout(25); await finish(page); },
  { family: 'foreign-callback', rules: ['reactive-write-in-owned-scope'], codes: ['REACTIVE_WRITE_IN_OWNED_SCOPE'] });
for (const mode of ['action', 'flush']) pair('lodash-' + mode, 'lodash', `import lodash from 'lodash';`,
  bad => `const [read, set] = createSignal(1); const run = action(function* () { ${mode === 'flush' && bad ? 'lodash.once(() => flush())();' : 'set(2);'} });
    ${mode === 'action' && bad ? 'lodash.once(() => run())();' : 'h.run = () => lodash.once(() => run())();'}`,
  () => `<p>action</p>`, async page => { await page.evaluate(async () => { const h = globalThis.__experiment;
    if (h.run) { try { await h.run(); } catch (error) { h.errors.push({ label: 'action', message: error.message, stack: error.stack }); } } }); await page.waitForTimeout(25); await finish(page); },
  { family: mode === 'flush' ? 'action-timing' : 'owned-action', rules: [mode === 'flush' ? 'flush-in-action' : 'action-called-in-owned-scope'],
    codes: mode === 'action' ? ['ACTION_CALLED_IN_OWNED_SCOPE'] : [], exceptionExpected: mode === 'flush' });
const latest = `import { createLatest } from '@solid-primitives/memo';`;
const pendingSetup = `const pending = createMemo(() => new Promise<number>(resolve => h.complete = resolve)); const value = createLatest([pending]);`;
const complete = async page => { await page.waitForTimeout(25); await page.evaluate(() => { const h = globalThis.__experiment;
  h.values.firstPaint = document.getElementById('root')?.textContent; h.complete?.(2); }); await page.waitForTimeout(40); await finish(page); };
pair('pending-read', '@solid-primitives/memo', latest,
  bad => pendingSetup + (bad ? ' const frozen = value();' : ''),
  bad => bad ? `<p>{String(frozen)}</p>` : `<Loading fallback={<p>waiting</p>}><p>{String(value())}</p></Loading>`, complete,
  { family: 'pending-async', rules: ['pending-async-unsuspendable-read'], codes: ['PENDING_ASYNC_UNTRACKED_READ'] });
pair('loading-boundary', '@solid-primitives/memo', latest,
  () => pendingSetup, bad => bad ? `<p>{String(value())}</p>` : `<Loading fallback={<p>waiting</p>}><p>{String(value())}</p></Loading>`, complete,
  { family: 'async-ui', rules: ['async-outside-loading-boundary'], codes: ['ASYNC_OUTSIDE_LOADING_BOUNDARY'],
    behaviorExpectation: 'show waiting UI during the first pending render' });
for (const [id, name, imports, registration, deliver] of [
  ['timer-life', '@solid-primitives/timer', `import { makeTimer } from '@solid-primitives/timer';`, `makeTimer(() => h.values.calls = (h.values.calls ?? 0) + 1, 10, setInterval)`, null],
  ['listener-life', '@solid-primitives/event-listener', `import { makeEventListener } from '@solid-primitives/event-listener';`, `makeEventListener(target, 'test', () => h.values.calls = (h.values.calls ?? 0) + 1)`, 'target.dispatchEvent(new Event("test"))'],
]) pair(id, name, imports,
  bad => `const target = new EventTarget(); const scope = (globalThis as any).__resourceAudit.scope('screen resources', 'stop');
    onCleanup(() => { h.clear?.(); queueMicrotask(() => scope.end()); });
    h.start = scope.bind(() => { const clear = ${registration}; ${bad ? '' : 'h.clear = clear;'} });
    ${deliver ? `h.deliver = () => ${deliver};` : ''}`,
  () => `<p>lifetime</p>`, async page => { await page.evaluate(() => globalThis.__experiment.start()); await page.waitForTimeout(25);
    await finish(page); await page.waitForTimeout(25); await page.evaluate(() => { const h = globalThis.__experiment;
      h.values.callsAtDispose = h.values.calls ?? 0; h.deliver?.(); h.values.resourceAudit = globalThis.__resourceAudit.snapshot(); }); await page.waitForTimeout(25); },
  { family: 'resource-lifetime', rules: [], codes: ['RESOURCE_OUTLIVES_DECLARED_SCOPE'], lifetimePolicy: 'screen resources stop on disposal' });
pair('computed-dispatch', '@solid-primitives/memo', `import * as M from '@solid-primitives/memo';`,
  bad => `const key = 'createReducer'; const [value, dispatch] = M[key]((state: number, increment: number) => state + increment, 1);
    h.update = () => { dispatch(1); flush(); }; ${bad ? 'const frozen = value();' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'value()'}}</p>`, update,
  { family: 'unknown-dispatch', rules: ['strict-read-untracked'], behaviorExpectation: 'render follows reducer dispatch', expectedGap: true });
cases.push({ id: 'family-intentional-snapshot-control', package: '@solid-primitives/memo', app: install('@solid-primitives/memo'),
  source: `${prelude}\n${reducer}\nfunction App() { ${reducerSetup} const frozen = value(); return <p id='value'>{frozen}</p>; }
  h.dispose = render(() => <App />, document.getElementById('root')!);`,
  flow: async page => { await page.evaluate(() => globalThis.__experiment.update()); await page.waitForTimeout(25);
    await page.evaluate(() => globalThis.__experiment.values.behavior = { desired: '1', actual: document.getElementById('value')?.textContent ?? null }); await finish(page); },
  provenance: { family: 'intent-control', role: 'control', expectedIssue: false, intentionalSnapshot: true, rules: [] } });
cases.push({ id: 'family-shadowed-factory-control', package: '@solid-primitives/memo', app: install('@solid-primitives/memo'),
  source: `${prelude}\n${reducer}\nfunction App() { function createReducer() { return [() => 1]; } const [value] = createReducer(); return <p>{value()}</p>; }
  h.dispose = render(() => <App />, document.getElementById('root')!);`, flow: settle,
  provenance: { family: 'symbol-control', role: 'control', expectedIssue: false, rules: [] } });
cases.push({ id: 'family-typescript-readonly-exclusion', package: '@solid-primitives/bounds', app: install('@solid-primitives/bounds'),
  source: `${prelude}\nimport { createElementBounds } from '@solid-primitives/bounds';
  function App() { const bounds = createElementBounds(document.body); bounds.width = 9; return <p>typing</p>; }
  h.dispose = render(() => <App />, document.getElementById('root')!);`, flow: settle,
  provenance: { family: 'type-exclusion', role: 'control', expectedIssue: false, expectedTypingCode: 2540, rules: [] } });
export default cases;
