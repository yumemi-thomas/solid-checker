// Authored after sealing v10; unsupported caller forms remain scored misses.
import './snapshot-feedback-v10.mjs';
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname), roots = read(resolve(repo, 'rust/target/primitives-checkpoint/run-browser.json')).results,
  cases = [], prelude = `import { flush, untrack } from 'solid-js'; import * as Solid from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function observe(desired, resize = false) {
  return async page => {
    if (resize) await page.setViewportSize({ width: 640, height: 900 });
    await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('update', h.update); }); await page.waitForTimeout(100);
    await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = {
      desired, actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; }, desired);
  };
}
function add(id, name, imports, setup, display, desired, resize, provenance) {
  cases.push({ id: 'snapshot-caller-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), roots.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return <p id='value'>{${display}}</p>; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow: observe(desired, resize), provenance });
}
function pair(id, name, imports, setup, expression, desired, resize = false) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports,
    `${setup} ${bad ? `const frozen = ${expression};` : ''}`, bad ? 'frozen' : expression, desired, resize,
    { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'observed-caller-snapshot',
      rules: ['strict-read-untracked'], codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, mouseSetup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
pair('untrack-first', '@solid-primitives/mouse', mouse, `${mouseSetup} function read() { return position['x']; } untrack(read);`, 'read()', '42');
pair('shorthand-escape', '@solid-primitives/mouse', mouse, `${mouseSetup} const read = () => position.x; h.holder = { read };`, 'read()', '42');
pair('wrapped-three-callers', '@solid-primitives/resize-observer', `import { createWindowSize } from '@solid-primitives/resize-observer';`,
  `const size = createWindowSize(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
   function width() { return size.width; } width(); untrack(width);`, '(width as () => number)()', '640', true);
pair('map-multiple-calls', '@solid-primitives/map', `import { ReactiveMap } from '@solid-primitives/map';`,
  `const state = new ReactiveMap<string, number>(); h.update = () => { state.set('n', 2); flush(); };
   const count = () => state.size; count(); untrack(count);`, 'count()', '1');
pair('set-escaped-arrow', '@solid-primitives/set', `import { ReactiveSet } from '@solid-primitives/set';`,
  `const state = new ReactiveSet<number>(); h.update = () => { state.add(2); flush(); };
   const count = () => state.size; h.count = count; untrack(count);`, 'count()', '1');
pair('shadowed-helper', '@solid-primitives/mouse', mouse, `${mouseSetup} function read() { return position.x; }
  { function read() { return 9; } read(); } untrack(read);`, 'read()', '42');
pair('member-caller', '@solid-primitives/mouse', mouse, `${mouseSetup} function read() { return position.x; } const holder = { read };`, 'holder.read()', '42');
pair('immutable-alias', '@solid-primitives/mouse', mouse, `${mouseSetup} function read() { return position.x; } const selected = read;`, 'selected()', '42');
pair('parameter-helper', '@solid-primitives/mouse', mouse, `${mouseSetup} function read(key: 'x') { return position[key]; }`, "read('x')", '42');
pair('forwarded-return', '@solid-primitives/mouse', mouse, `${mouseSetup} const inner = () => position.x; function read() { return inner(); }`, 'read()', '42');
for (const [id, helper, initializer, desired] of [
  ['unexecuted-caller', `function read() { return position.x; } untrack(read);
    function NeverMounted() { const missed = read(); return <b>{missed}</b>; }`, '9', '9'],
  ['explicit-two-callers', `function read() { return position.x; } read();`, 'untrack(read)', '0'],
  ['explicit-namespace', `const read = () => position.x; h.read = read;`, 'Solid.untrack(read)', '0'],
  ['explicit-member', `function read() { return position.x; } const holder = { read };`, 'untrack(() => holder.read())', '0'],
  ['ignored-argument', `function read() { return position.x; } function ignore(_value: number) { return 9; }`, 'ignore(read())', '9'],
  ['deferred-helper', `function read() { return position.x; }`, '() => read()', '42'],
  ['different-helper', `function read() { return position.x; } function other() { return 9; } untrack(read);`, 'other()', '9'],
  ['unknown-return-call', `function ignore(_value: number) { return { x: 9 }; } function read() { return ignore(position.x).x; } h.read = read;`, 'read()', '9'],
]) add(id + '-control', '@solid-primitives/mouse', mouse, `${mouseSetup} ${helper} const frozen = ${initializer};`,
  id === 'deferred-helper' ? 'frozen()' : 'frozen', desired, false,
  { family: 'caller-context-control', role: 'control', expectedIssue: false, rules: [] });
for (const [id, setup, code] of [
  ['function-assignment', `function read() { return position.x; } read = () => 9; const frozen = read();`, 2630],
  ['missing-published-property', `const frozen = position.absentCoordinate;`, 2339],
]) add(id + '-type-exclusion', '@solid-primitives/mouse', mouse, `${mouseSetup} ${setup}`, 'frozen', '9', false,
  { family: 'published-type-exclusion', role: 'type-exclusion', expectedIssue: false, rules: [], expectedTypingCode: code });
export default cases;
