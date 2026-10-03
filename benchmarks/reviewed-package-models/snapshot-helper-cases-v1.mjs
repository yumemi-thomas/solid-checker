// Authored after sealing v9. Both supported and ambiguous helpers stay in the score.
import './snapshot-feedback-v9.mjs';
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
  cases.push({ id: 'snapshot-helper-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), roots.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return <p id='value'>{${display}}</p>; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow: observe(desired, resize), provenance });
}
function pair(id, name, imports, setup, expression, desired, resize = false) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports,
    `${setup(bad)} ${bad ? `const frozen = ${expression};` : ''}`, bad ? 'frozen' : expression, desired, resize,
    { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'local-helper-snapshot',
      rules: ['strict-read-untracked'], codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, setup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
pair('arrow', '@solid-primitives/mouse', mouse, () => `${setup} const read = () => position['x'];`, 'read()', '42');
pair('transparent-return', '@solid-primitives/mouse', mouse,
  () => `${setup} function read() { return (position.x as number); }`, 'read()', '42');
pair('resize-return', '@solid-primitives/resize-observer', `import { createWindowSize as dimensions } from '@solid-primitives/resize-observer';`,
  () => `const size = dimensions(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
    function width() { return size['width']; }`, 'width()', '640', true);
pair('map-size-return', '@solid-primitives/map', `import { ReactiveMap } from '@solid-primitives/map';`,
  () => `const state = new ReactiveMap<string, number>(); h.update = () => { state.set('n', 2); flush(); };
    function count() { return state.size; }`, 'count()', '1');
pair('multiple-callers', '@solid-primitives/mouse', mouse,
  bad => `${setup} function read() { return position['x']; } ${bad ? 'read();' : 'untrack(read);'}`, 'read()', '42');
pair('escaped-helper', '@solid-primitives/mouse', mouse,
  () => `${setup} function read() { return position['x']; } h.read = read;`, 'read()', '42');
for (const [id, helper, initializer, desired] of [
  ['ignored-return-argument', `function ignore(_value: number) { return { x: 9 }; } function read() { return ignore(position['x']).x; }`, 'read()', '9'],
  ['discarded-return', `function read() { return (position['x'], 9); }`, 'read()', '9'],
  ['explicit-helper', `function read() { return position['x']; }`, 'untrack(read)', '0'],
  ['explicit-escaped-helper', `const read = () => position['x']; h.read = read;`, 'untrack(read)', '0'],
]) add(id + '-control', '@solid-primitives/mouse', mouse, `${setup} ${helper} const frozen = ${initializer};`, 'frozen', desired, false,
  { family: 'helper-value-flow-control', role: 'control', expectedIssue: false, rules: [] });
export default cases;
