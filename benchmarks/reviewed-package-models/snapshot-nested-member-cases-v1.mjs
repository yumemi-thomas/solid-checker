// Authored after sealing v13; borrowed receiver objects remain scored misses.
import './snapshot-feedback-v13.mjs';
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname), roots = read(resolve(repo, 'rust/target/primitives-checkpoint/run-browser.json')).results,
  cases = [], prelude = `import { flush, untrack } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function observe(desired, resize = false) {
  return async page => {
    if (resize) await page.setViewportSize({ width: 640, height: 900 });
    await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('update', h.update); }); await page.waitForTimeout(100);
    await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = {
      desired, actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; }, desired);
  };
}
function add(id, name, imports, setup, display, desired, resize, provenance) {
  cases.push({ id: 'snapshot-nested-member-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), roots.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return <p id='value'>{${display}}</p>; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow: observe(desired, resize), provenance });
}
function pair(id, name, imports, setup, expression, desired, resize = false) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports,
    `${setup} ${bad ? `const frozen = ${expression};` : ''}`, bad ? 'frozen' : expression, desired, resize,
    { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'nested-own-member-return', rules: ['strict-read-untracked'],
      codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, mouseSetup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
pair('computed-path', '@solid-primitives/mouse', mouse,
  `${mouseSetup} const read = () => position.x; const holder = { api: { read } }; const section = 'api'; const operation = 'read';`, 'holder[section][operation]()', '42');
pair('nested-seven-helper-path', '@solid-primitives/mouse', mouse,
  `${mouseSetup} const holder = { api: { read() { return position.x; } } }; function step1() { return holder.api.read(); }
   ${Array.from({ length: 5 }, (_, index) => `function step${index + 2}() { return step${index + 1}(); }`).join(' ')}`, 'step6()', '42');
pair('resize-child-alias', '@solid-primitives/resize-observer', `import { createWindowSize } from '@solid-primitives/resize-observer';`,
  `const size = createWindowSize(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
   const holder = { api: { read(key: 'width') { return size[key]; } } }; const selected = holder.api;`, "selected.read('width')", '640', true);
pair('map-nested-arrow', '@solid-primitives/map', `import { ReactiveMap } from '@solid-primitives/map';`,
  `const state = new ReactiveMap<string, number>(); h.update = () => { state.set('n', 2); flush(); }; const holder = { api: { read: () => state.size } };`, 'holder.api.read()', '1');
pair('set-extracted-nested-method', '@solid-primitives/set', `import { ReactiveSet } from '@solid-primitives/set';`,
  `const state = new ReactiveSet<number>(); h.update = () => { state.add(2); flush(); };
   const holder = { api: { read() { return state.size; } } }; const selected = holder.api.read; const get = selected;`, 'get()', '1');
pair('borrowed-receiver', '@solid-primitives/mouse', mouse,
  `${mouseSetup} const read = () => position.x; const api = { read }; const holder = { api };`, 'holder.api.read()', '42');
for (const [id, setup, initializer, desired] of [
  ['child-object-replacement', `const read = () => position.x; const holder = { api: { read } }; holder.api.read(); holder.api = { read: () => 9 };`, 'holder.api.read()', '9'],
  ['child-alias-replacement', `const read = () => position.x; const holder = { api: { read } }; holder.api.read(); const selected = holder.api; selected.read = () => 9;`, 'holder.api.read()', '9'],
  ['child-argument-replacement', `const read = () => position.x; const holder = { api: { read } }; holder.api.read();
    function replace(value: { read: () => number }) { value.read = () => 9; } replace(holder.api);`, 'holder.api.read()', '9'],
  ['nested-accessor-snapshot', `const read = () => position.x; const holder = { get api() { return {read}; } };`, 'untrack(() => holder.api.read())', '0'],
  ['discarded-nested-return', `const holder = { api: { read() { return position.x; } } }; function outer() { return (holder.api.read(), 9); }`, 'outer()', '9'],
  ['explicit-nested-chain', `const read = () => position.x; const holder = { api: { read } }; function outer() { return holder.api.read(); }`, 'untrack(outer)', '0'],
]) add(id + '-control', '@solid-primitives/mouse', mouse, `${mouseSetup} ${setup} const frozen = ${initializer};`, 'frozen', desired, false,
  { family: 'nested-member-precision-control', role: 'control', expectedIssue: false, rules: [] });
for (const [id, setup, code] of [
  ['missing-member', `const holder = { api: { read() { return position.x; } } }; const frozen = holder.api.absent();`, 2339],
  ['readonly-nested-member', `const holder = { api: { read: () => position.x } } as const; holder.api.read = () => 9; const frozen = holder.api.read();`, 2540],
]) add(id + '-type-exclusion', '@solid-primitives/mouse', mouse, `${mouseSetup} ${setup}`, 'frozen', '9', false,
  { family: 'published-type-exclusion', role: 'type-exclusion', expectedIssue: false, rules: [], expectedTypingCode: code });
export default cases;
