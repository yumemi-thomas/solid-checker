// Authored after sealing v12. Escapes and nested receivers remain in the score.
import './snapshot-feedback-v12.mjs';
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
  cases.push({ id: 'snapshot-member-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), roots.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return <p id='value'>{${display}}</p>; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow: observe(desired, resize), provenance });
}
function pair(id, name, imports, setup, expression, desired, resize = false) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports,
    `${setup} ${bad ? `const frozen = ${expression};` : ''}`, bad ? 'frozen' : expression, desired, resize,
    { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'stable-member-and-deep-return', rules: ['strict-read-untracked'],
      codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, mouseSetup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
pair('literal-member', '@solid-primitives/mouse', mouse, `${mouseSetup} const read = () => position.x; const holder = { read };`, "holder['read']()", '42');
pair('inline-method', '@solid-primitives/mouse', mouse, `${mouseSetup} const holder = { read() { return position.x; } };`, 'holder.read()', '42');
pair('inline-arrow', '@solid-primitives/mouse', mouse, `${mouseSetup} const holder = { read: () => position.x };`, 'holder.read()', '42');
pair('constant-key', '@solid-primitives/mouse', mouse, `${mouseSetup} const read = () => position.x; const holder = { read }; const key = 'read';`, 'holder[key]()', '42');
pair('receiver-and-function-alias', '@solid-primitives/mouse', mouse, `${mouseSetup} const read = () => position.x; const holder = { value: read }; const selected = holder; const get = selected.value;`, 'get()', '42');
pair('returned-member-call', '@solid-primitives/mouse', mouse, `${mouseSetup} const holder = { read() { return position.x; } }; function outer() { return holder.read(); }`, 'outer()', '42');
pair('resize-method-parameter', '@solid-primitives/resize-observer', `import { createWindowSize } from '@solid-primitives/resize-observer';`,
  `const size = createWindowSize(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
   const holder = { read(key: 'width') { return size[key]; } };`, "holder.read('width')", '640', true);
function helpers(count, leaf) {
  return `function step1() { return ${leaf}; } ` + Array.from({ length: count - 1 }, (_, index) => `function step${index + 2}() { return step${index + 1}(); }`).join(' ');
}
pair('map-six-helpers', '@solid-primitives/map', `import { ReactiveMap } from '@solid-primitives/map';`,
  `const state = new ReactiveMap<string, number>(); h.update = () => { state.set('n', 2); flush(); }; ${helpers(6, 'state.size')}`, 'step6()', '1');
pair('set-twelve-helpers', '@solid-primitives/set', `import { ReactiveSet } from '@solid-primitives/set';`,
  `const state = new ReactiveSet<number>(); h.update = () => { state.add(2); flush(); }; ${helpers(12, 'state.size')}`, 'step12()', '1');
pair('passed-object-method', '@solid-primitives/mouse', mouse, `${mouseSetup} const holder = { read(value: ReturnType<typeof createMousePosition>) { return value['x']; } };`, 'holder.read(position)', '42');
pair('escaped-receiver', '@solid-primitives/mouse', mouse, `${mouseSetup} const read = () => position.x; const holder = { read }; h.holder = holder;`, 'holder.read()', '42');
pair('nested-receiver', '@solid-primitives/mouse', mouse, `${mouseSetup} const read = () => position.x; const holder = { api: { read } };`, 'holder.api.read()', '42');
for (const [id, setup, initializer, display, desired] of [
  ['replacement', `const read = () => position.x; const holder = { read }; holder.read(); holder.read = () => 9;`, 'holder.read()', 'frozen', '9'],
  ['alias-replacement', `const read = () => position.x; const holder = { read }; holder.read(); const selected = holder; selected.read = () => 9;`, 'holder.read()', 'frozen', '9'],
  ['escaped-replacement', `const read = () => position.x; const holder = { read }; holder.read(); h.holder = holder; h.holder.read = () => 9;`, 'holder.read()', 'frozen', '9'],
  ['spread-snapshot', `const read = () => position.x; const holder = { read }; const copied = { ...holder };`, 'untrack(() => copied.read())', 'frozen', '0'],
  ['accessor-snapshot', `const read = () => position.x; const holder = { get read() { return read; } };`, 'untrack(() => holder.read())', 'frozen', '0'],
  ['ignored-argument', `const holder = { read() { return position.x; } }; function ignore(_value: number) { return 9; }`, 'ignore(holder.read())', 'frozen', '9'],
  ['discard-before-deferred', `const holder = { read() { return position.x; } }; holder.read();`, '() => holder.read()', 'frozen()', '42'],
  ['explicit-two-callers', `const read = () => position.x; const holder = { read }; untrack(() => holder.read());`, 'untrack(() => holder.read())', 'frozen', '0'],
]) add(id + '-control', '@solid-primitives/mouse', mouse, `${mouseSetup} ${setup} const frozen = ${initializer};`, display, desired, false,
  { family: 'member-precision-control', role: 'control', expectedIssue: false, rules: [] });
for (const [id, setup, code] of [
  ['wrong-method-key', `const holder = { read(key: 'x') { return position[key]; } }; const frozen = holder.read('absent');`, 2345],
  ['readonly-member', `const holder = { read: () => position.x } as const; holder.read = () => 9; const frozen = holder.read();`, 2540],
]) add(id + '-type-exclusion', '@solid-primitives/mouse', mouse, `${mouseSetup} ${setup}`, 'frozen', '9', false,
  { family: 'published-type-exclusion', role: 'type-exclusion', expectedIssue: false, rules: [], expectedTypingCode: code });
export default cases;
