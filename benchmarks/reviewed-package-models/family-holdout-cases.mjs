// Held out of the combined matrix. Expectations are evaluation data only.
// No consumer or detector is rewritten after this population is executed.
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname);
const freeze = read(resolve(repo, 'rust/target/family-holdout-detector-freeze.json'));
const cases = [], prelude = `import { createSignal, createMemo, createTrackedEffect, Loading, action, flush, untrack } from 'solid-js';
import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function install(name) {
  const row = freeze.packages.find(row => row.package === name); if (!row) throw new Error(name);
  return relative(resolve(repo, 'rust/target/app-import-metric/apps'), row.project);
}
const finish = async page => page.evaluate(() => { const h = globalThis.__experiment; h.dispose?.(); h.disposals++; });
const settle = async page => { await page.waitForTimeout(35); await finish(page); };
const invoke = async page => { await page.evaluate(() => { const h = globalThis.__experiment; if (h.run) h.attempt('invoke', h.run); }); await page.waitForTimeout(35); await finish(page); };
function observe(desired, update = true) {
  return async page => {
    if (update) await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('update', h.update); });
    await page.waitForTimeout(60);
    await page.evaluate(desired => { const h = globalThis.__experiment;
      h.values.behavior = { desired, actual: document.getElementById('value')?.textContent ?? null }; }, desired);
    await finish(page);
  };
}
function pair(id, name, imports, setup, jsx, flow, expected) {
  for (const bad of [true, false]) cases.push({ id: `holdout-${id}-${bad ? 'target' : 'control'}`, package: name, app: install(name),
    source: `${prelude}\n${imports}\nfunction App() { ${setup(bad)} return ${jsx(bad)}; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`, flow,
    provenance: { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, ...expected } });
}
const strict = { family: 'silent-staleness', rules: ['strict-read-untracked'], codes: ['STRICT_READ_UNTRACKED', 'SOURCE_GETTER_SNAPSHOT_FLOW'] };
const accessor = { family: 'accessor-value', rules: ['uncalled-accessor'], codes: [] };
const write = { family: 'callback-owned-write', rules: ['reactive-write-in-owned-scope'], codes: ['REACTIVE_WRITE_IN_OWNED_SCOPE'] };
for (const computed of [false, true]) pair(computed ? 'map-computed-snapshot' : 'map-snapshot', '@solid-primitives/map',
  `import { ReactiveMap } from '@solid-primitives/map';`,
  bad => `const state = new ReactiveMap<string, number>([['n', 1]]); h.update = () => { state.set('n', 2); flush(); };
    ${bad ? `const frozen = ${computed ? "state['get']('n')" : "state.get('n')"};` : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : "state.get('n')"}}</p>`, observe('2'),
  { ...strict, family: computed ? 'computed-member-staleness' : 'class-method-staleness' });
pair('set-size-snapshot', '@solid-primitives/set', `import { ReactiveSet } from '@solid-primitives/set';`,
  bad => `const state = new ReactiveSet([1]); h.update = () => { state.add(2); flush(); }; ${bad ? 'const frozen = state.size;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'state.size'}}</p>`, observe('2'), { ...strict, family: 'class-getter-staleness' });
pair('set-union-accessor', '@solid-primitives/set', `import { ReactiveSet, union } from '@solid-primitives/set';`,
  () => `const left = new ReactiveSet([1]), right = new ReactiveSet([2]); const combined = union(left, right);`,
  bad => `<p id='value'>{${bad ? '`set:${combined}`' : '`set:${[...combined()].join(",")}`'}}</p>`, observe('set:1,2', false), accessor);
pair('leading-owned-write', '@solid-primitives/scheduled', `import { leading, throttle } from '@solid-primitives/scheduled';`,
  bad => `const [, set] = createSignal(1); const run = leading(throttle, () => set(2), 10); ${bad ? 'run();' : 'h.run = run;'}`,
  () => '<p>leading callback</p>', invoke, write);
pair('bus-owned-write', '@solid-primitives/event-bus', `import { createEventBus } from '@solid-primitives/event-bus';`,
  bad => `const [, set] = createSignal(1); const bus = createEventBus<number>(); bus.listen(value => set(value));
    ${bad ? 'bus.emit(2);' : 'h.run = () => bus.emit(2);'}`, () => '<p>event listener</p>', invoke, write);
pair('bus-owned-action', '@solid-primitives/event-bus', `import { createEventBus } from '@solid-primitives/event-bus';`,
  bad => `const [, set] = createSignal(1); const mutate = action(function* () { set(2); });
    const bus = createEventBus<void>(); bus.listen(() => { mutate(); }); ${bad ? 'bus.emit();' : 'h.run = () => bus.emit();'}`,
  () => '<p>event action</p>', invoke,
  { family: 'callback-owned-action', rules: ['action-called-in-owned-scope'], codes: ['ACTION_CALLED_IN_OWNED_SCOPE'] });
const date = `import { createDate } from '@solid-primitives/date';`;
pair('date-snapshot', '@solid-primitives/date', date,
  bad => `const [value, set] = createDate(1000); h.update = () => { set(2000); flush(); }; ${bad ? 'const frozen = value().getTime();' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'value().getTime()'}}</p>`, observe('2000'), strict);
pair('date-accessor', '@solid-primitives/date', date, () => 'const [value] = createDate(1000);',
  bad => `<p id='value'>{${bad ? '`date:${value}`' : '`date:${value().getTime()}`'}}</p>`, observe('date:1000', false), accessor);
pair('mouse-snapshot', '@solid-primitives/mouse', `import { createMousePosition } from '@solid-primitives/mouse';`,
  bad => `const position = createMousePosition(window, { touch: false }); h.update = () => {
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); }; ${bad ? 'const frozen = position.x;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'position.x'}}</p>`, observe('42'), strict);
const controlled = `import { createControllableSignal } from '@solid-primitives/controlled-signal';`;
pair('controlled-snapshot', '@solid-primitives/controlled-signal', controlled,
  bad => `const [value, set] = createControllableSignal({ defaultValue: () => 1 }); h.update = () => { set(2); flush(); }; ${bad ? 'const frozen = value();' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'value()'}}</p>`, observe('2'), strict);
pair('controlled-onchange-write', '@solid-primitives/controlled-signal', controlled,
  bad => `const [external, setExternal] = createSignal(1); const [, set] = createControllableSignal({ value: external, onChange: setExternal });
    ${bad ? 'set(2);' : 'h.run = () => { set(2); flush(); };'}`, () => '<p>controlled callback</p>', invoke, write);
const segment = `import { createSegment } from '@solid-primitives/pagination';`;
pair('segment-accessor', '@solid-primitives/pagination', segment,
  () => 'const value = createSegment([1, 2, 3], 2, () => 1);',
  bad => `<p id='value'>{${bad ? '`page:${value}`' : '`page:${value().join(",")}`'}}</p>`, observe('page:1,2', false), accessor);
pair('segment-frozen-input', '@solid-primitives/pagination', segment,
  bad => `const [items, setItems] = createSignal([1, 2]); const value = createSegment(${bad ? 'items()' : 'items'}, 2, () => 1);
    h.update = () => { setItems([3, 4]); flush(); };`, () => '<p id="value">{value().join(",")}</p>', observe('3,4'),
  { ...strict, family: 'frozen-package-input' });
pair('history-compute-write', '@solid-primitives/history', `import { createUndoHistory } from '@solid-primitives/history';`,
  bad => `const [value, set] = createSignal(1); const history = createUndoHistory(() => {
    ${bad ? 'set(2);' : ''} const snapshot = value(); return () => set(snapshot); });`,
  () => '<p id="value">history ready</p>', settle, write);
pair('retry-read-after-await', '@solid-primitives/promise', `import { retry } from '@solid-primitives/promise';`,
  bad => `const [count, set] = createSignal(1); h.update = () => { set(2); flush(); };
    const value = createMemo(() => retry(async () => { ${bad ? 'await Promise.resolve(); return count();' : 'const captured = count(); await Promise.resolve(); return captured;'} }, { times: 1 }));`,
  () => '<Loading fallback={<p>waiting</p>}><p id="value">{String(value())}</p></Loading>',
  async page => { await page.waitForFunction(() => document.getElementById('value')?.textContent === '1', null, { timeout: 5000 }); await observe('2')(page); },
  { family: 'external-async-callback', rules: ['reactive-read-after-await'], codes: [] });
pair('neverthrow-owned-write', 'neverthrow', `import { ok } from 'neverthrow';`,
  bad => `const [, set] = createSignal(1); const run = () => ok(1).map(value => { set(value + 1); return value; });
    ${bad ? 'run();' : 'h.run = run;'}`, () => '<p>result callback</p>', invoke, write);
pair('zod-owned-write', 'zod', `import { z } from 'zod';`,
  bad => `const [, set] = createSignal(1); const schema = z.number().transform(value => { set(value + 1); return value; });
    ${bad ? 'schema.parse(1);' : 'h.run = () => schema.parse(1);'}`, () => '<p>schema callback</p>', invoke, write);
// Timing and explicit snapshot controls challenge false-positive suppression.
for (const [id, name, imports, setup, jsx, flow] of [
  ['trailing-owned-trigger', '@solid-primitives/scheduled', `import { debounce } from '@solid-primitives/scheduled';`,
    'const [value, set] = createSignal(1); const run = debounce(() => set(2), 10); run();', '<p id="value">{value()}</p>', observe('2', false)],
  ['explicit-class-snapshot', '@solid-primitives/map', `import { ReactiveMap } from '@solid-primitives/map';`,
    `const state = new ReactiveMap<string, number>([['n', 1]]); const frozen = untrack(() => state.get('n'));
    h.update = () => { state.set('n', 2); flush(); };`, '<p id="value">{frozen}</p>', observe('1')],
]) cases.push({ id: 'holdout-' + id + '-control', package: name, app: install(name),
  source: `${prelude}\n${imports}\nfunction App() { ${setup} return ${jsx}; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`, flow,
  provenance: { family: 'precision-control', role: 'control', expectedIssue: false, rules: [] } });
// Real published signatures must reject these. The checker must stay silent.
for (const [id, name, imports, statement, code] of [
  ['readonly-set', '@solid-primitives/set', `import { ReactiveSet, readonlySet } from '@solid-primitives/set';`,
    'const state = readonlySet(new ReactiveSet([1])); state.add(2);', 2339],
  ['date-invalid-input', '@solid-primitives/date', date, 'createDate(true);', 2345],
]) cases.push({ id: 'holdout-' + id + '-typing-exclusion', package: name, app: install(name),
  source: `${prelude}\n${imports}\nfunction App() { ${statement} return <p>typing</p>; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`, flow: settle,
  provenance: { family: 'type-exclusion', role: 'control', expectedIssue: false, rules: [], expectedTypingCode: code } });
export default cases;
