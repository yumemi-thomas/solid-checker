// Fresh consumers authored after snapshot-v4-detector-freeze.json was sealed.
// Expectations belong to evaluation and never enter the detector.
import './snapshot-feedback-v4.mjs';
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname),
  installs = read(resolve(repo, 'rust/target/primitives-checkpoint/run-browser.json')).results;
const cases = [], prelude = `import { createMemo, flush, untrack } from 'solid-js';
import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function app(name) {
  return relative(resolve(repo, 'rust/target/app-import-metric/apps'), installs.find(row => row.package === name).retainedArtifacts.projectDir);
}
async function finish(page) { await page.evaluate(() => { const h = globalThis.__experiment; h.dispose?.(); h.disposals++; }); }
function observe(desired, resize = false) {
  return async page => {
    if (resize) await page.setViewportSize({ width: 640, height: 900 });
    await page.evaluate(() => { const h = globalThis.__experiment; if (h.update) h.attempt('update', h.update); });
    await page.waitForTimeout(90);
    await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = {
      desired, actual: document.getElementById('value')?.textContent ?? null }; }, desired);
    await finish(page);
  };
}
function add(id, name, imports, setup, jsx, flow, provenance) {
  cases.push({ id: `snapshot-fresh-${id}`, package: name, app: app(name),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return ${jsx}; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`, flow, provenance });
}
function pair(id, name, imports, setup, jsx, flow) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports, setup(bad), jsx(bad), flow,
    { pair: id, role: bad ? 'target' : 'control', family: 'fresh-snapshot', expectedIssue: bad,
      rules: ['strict-read-untracked'], codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW'] });
}
const mapImport = `import { ReactiveMap, ReactiveWeakMap } from '@solid-primitives/map';`,
  setImport = `import { ReactiveWeakSet } from '@solid-primitives/set';`,
  mouseImport = `import { createMousePosition } from '@solid-primitives/mouse';`,
  mapSetup = `const state = new ReactiveMap<string, number>([['n', 1]]); h.update = () => { state.set('n', 2); flush(); };`,
  mouseSetup = `const position = createMousePosition(window, { touch: false }); h.update = () => {
    window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
pair('weakmap', '@solid-primitives/map', mapImport,
  bad => `const key = {}; const state = new ReactiveWeakMap<object, number>([[key, 1]]);
    h.update = () => { state.set(key, 2); flush(); }; ${bad ? 'const frozen = state.get(key);' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'state.get(key)'}}</p>`, observe('2'));
pair('weakset', '@solid-primitives/set', setImport,
  bad => `const key = {}; const state = new ReactiveWeakSet<object>(); h.update = () => { state.add(key); flush(); };
    ${bad ? 'const frozen = state.has(key);' : ''}`,
  bad => `<p id='value'>{String(${bad ? 'frozen' : 'state.has(key)'})}</p>`, observe('true'));
pair('const-key', '@solid-primitives/map', mapImport,
  bad => `${mapSetup} const member = 'get'; ${bad ? 'const frozen = state[member]("n");' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'state[member]("n")'}}</p>`, observe('2'));
pair('namespace-wrapped', '@solid-primitives/set', `import * as sets from '@solid-primitives/set';`,
  bad => `const state = new sets.ReactiveSet([1]); h.update = () => { state.add(2); flush(); };
    ${bad ? 'const frozen = (state as sets.ReactiveSet<number>).size;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'state.size'}}</p>`, observe('2'));
pair('single-destructure', '@solid-primitives/mouse', mouseImport,
  bad => `${mouseSetup} ${bad ? 'const { x: frozen } = position;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'position.x'}}</p>`, observe('42'));
pair('scroll', '@solid-primitives/scroll', `import { createScrollPosition } from '@solid-primitives/scroll';`,
  bad => `const element = document.createElement('div'); element.style.cssText = 'height:20px;width:20px;overflow:scroll';
    element.innerHTML = '<div style="height:200px">scroll</div>'; document.body.append(element);
    const position = createScrollPosition(element); h.update = () => { element.scrollTop = 30; element.dispatchEvent(new Event('scroll')); flush(); };
    ${bad ? 'const frozen = position.y;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'position.y'}}</p>`, observe('30'));
pair('window-size', '@solid-primitives/resize-observer', `import { createWindowSize } from '@solid-primitives/resize-observer';`,
  bad => `const size = createWindowSize(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
    ${bad ? 'const frozen = size.width;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'size.width'}}</p>`, observe('640', true));
pair('breakpoints', '@solid-primitives/media', `import { createBreakpoints } from '@solid-primitives/media';`,
  bad => `const matches = createBreakpoints({ wide: '1000px' }); h.update = () => flush();
    ${bad ? 'const frozen = matches.wide;' : ''}`,
  bad => `<p id='value'>{String(${bad ? 'frozen' : 'matches.wide'})}</p>`, observe('false', true));
// These broader shapes stay in the denominator even when the detector refuses them.
pair('dynamic-key', '@solid-primitives/map', mapImport,
  bad => `${mapSetup} const member = (() => 'get' as const)(); ${bad ? 'const frozen = state[member]("n");' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'state[member]("n")'}}</p>`, observe('2'));
pair('multiple-destructure', '@solid-primitives/mouse', mouseImport,
  bad => `${mouseSetup} ${bad ? 'const { x, y } = position;' : ''}`,
  bad => `<p id='value'>{${bad ? 'x + y' : 'position.x + position.y'}}</p>`, observe('47'));
const controls = [
  ['tracked-callback', '@solid-primitives/map', mapImport,
    `${mapSetup} const view = createMemo(() => { const value = state.get('n'); return <p id='value'>{value}</p>; });`, '<>{view()}</>', observe('2')],
  ['explicit-getter', '@solid-primitives/mouse', mouseImport,
    `${mouseSetup} const frozen = untrack(() => position.x);`, `<p id='value'>{frozen}</p>`, observe('0')],
  ['replaced-member', '@solid-primitives/map', mapImport,
    `${mapSetup} state.get = () => 9; const frozen = state.get('n');`, `<p id='value'>{frozen}</p>`, observe('9')],
  ['escaped-instance', '@solid-primitives/map', mapImport,
    `${mapSetup} const replace = (value: ReactiveMap<string, number>) => { value.get = () => 9; }; replace(state); const frozen = state.get('n');`,
    `<p id='value'>{frozen}</p>`, observe('9')],
  ['prototype-inspected', '@solid-primitives/map', mapImport,
    `${mapSetup} h.values.prototype = typeof ReactiveMap.prototype.get; const frozen = state.get('n');`, `<p id='value'>{frozen}</p>`, observe('1')],
  ['local-shadow', '@solid-primitives/map', '',
    `class ReactiveMap { get(_key: string) { return 9; } } const state = new ReactiveMap(); const frozen = state.get('n');`,
    `<p id='value'>{frozen}</p>`, observe('9')],
  // The code has no intent marker. A useful hint for a bug is noise here.
  ['implicit-intent', '@solid-primitives/map', mapImport,
    `${mapSetup} const frozen = state.get('n');`, `<p id='value'>{frozen}</p>`, observe('1')],
];
for (const [id, name, imports, setup, jsx, flow] of controls) add(id + '-control', name, imports, setup, jsx, flow,
  { family: id === 'implicit-intent' ? 'undeclared-intent' : 'precision-control', role: 'control', expectedIssue: false, rules: [] });
for (const [id, name, imports, setup] of [
  ['readonly-window-size', '@solid-primitives/resize-observer', `import { createWindowSize } from '@solid-primitives/resize-observer';`, 'const size = createWindowSize(); size.width = 12;'],
  ['readonly-scroll', '@solid-primitives/scroll', `import { createScrollPosition } from '@solid-primitives/scroll';`, 'const position = createScrollPosition(window); position.x = 12;'],
]) add(id + '-typing-exclusion', name, imports, setup, '<p>typing</p>', observe('typing'),
  { family: 'type-exclusion', role: 'control', expectedIssue: false, rules: [], expectedTypingCode: 2540 });
export default cases;
