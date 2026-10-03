// Authored after the v5 detector was sealed; no detector tuning follows this run.
import './snapshot-feedback-v5.mjs';
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname), installs = read(resolve(repo, 'rust/target/primitives-checkpoint/run-browser.json')).results,
  cases = [], prelude = `import { flush, untrack } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function flow(desired, resize = false) {
  return async page => {
    if (resize) await page.setViewportSize({ width: 640, height: 900 });
    await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('update', h.update); }); await page.waitForTimeout(90);
    await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = {
      desired, actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; }, desired);
  };
}
function add(id, name, imports, setup, jsx, desired, resize, provenance) {
  cases.push({ id: 'snapshot-transfer-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), installs.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return ${jsx}; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`, flow: flow(desired, resize), provenance });
}
for (const bad of [true, false]) {
  const provenance = pair => ({ pair, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'post-refinement-transfer',
    rules: ['strict-read-untracked'], codes: ['SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
  add('weakmap-call-key-' + (bad ? 'target' : 'control'), '@solid-primitives/map', `import { ReactiveWeakMap } from '@solid-primitives/map';`,
    `const key = {}; const state = new ReactiveWeakMap<object, number>([[key, 3]]); const member = () => 'get' as const;
      h.update = () => { state.set(key, 7); flush(); }; ${bad ? 'const frozen = state[member()](key);' : ''}`,
    `<p id='value'>{${bad ? 'frozen' : 'state[member()](key)'}}</p>`, '7', false, provenance('weakmap-call-key'));
  add('aliased-size-bindings-' + (bad ? 'target' : 'control'), '@solid-primitives/resize-observer',
    `import { createWindowSize as windowDimensions } from '@solid-primitives/resize-observer';`,
    `const dimensions = windowDimensions(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
      ${bad ? 'const { width: wide, height: tall } = dimensions;' : ''}`,
    `<p id='value'>{${bad ? 'wide + tall' : 'dimensions.width + dimensions.height'}}</p>`, '1540', true, provenance('aliased-size-bindings'));
  // A computed property getter is a distinct unsupported shape; retain the miss.
  add('computed-property-' + (bad ? 'target' : 'control'), '@solid-primitives/mouse', `import { createMousePosition } from '@solid-primitives/mouse';`,
    `const position = createMousePosition(window, { touch: false }); h.update = () => {
      window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };
      ${bad ? "const frozen = position['x'];" : ''}`,
    `<p id='value'>{${bad ? 'frozen' : "position['x']"}}</p>`, '42', false, provenance('computed-property'));
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, setup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
for (const [id, statement] of [
  ['default-binding', 'const { x = 99, y } = position;'],
  ['rest-binding', 'const { x, ...rest } = position; const y = 0;'],
  ['explicit-callback', 'const [x, y] = untrack(() => { const { x, y } = position; return [x, y]; });'],
]) add(id + '-control', '@solid-primitives/mouse', mouse, setup + statement, `<p id='value'>{x + y}</p>`, '0', false,
  { family: 'conservative-shape-control', role: 'control', expectedIssue: false, rules: [] });
export default cases;
