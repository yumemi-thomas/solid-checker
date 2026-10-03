// New population authored after sealing v11; the depth limit stays visible.
import './snapshot-feedback-v11.mjs';
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
  cases.push({ id: 'snapshot-local-path-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), roots.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return <p id='value'>{${display}}</p>; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow: observe(desired, resize), provenance });
}
function pair(id, name, imports, setup, expression, desired, resize = false) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports,
    `${setup} ${bad ? `const frozen = ${expression};` : ''}`, bad ? 'frozen' : expression, desired, resize,
    { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'local-return-path', rules: ['strict-read-untracked'],
      codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, mouseSetup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`;
pair('three-helpers-with-aliases', '@solid-primitives/mouse', mouse,
  `${mouseSetup} const leaf = () => position['x']; const selectedLeaf = leaf;
   function middle() { return selectedLeaf(); } const selectedMiddle = middle; function outer() { return selectedMiddle(); }`, 'outer()', '42');
pair('passed-object', '@solid-primitives/mouse', mouse,
  `${mouseSetup} function read(value: ReturnType<typeof createMousePosition>) { return value['x']; }`, 'read(position)', '42');
pair('wrapped-return-and-setup', '@solid-primitives/mouse', mouse,
  `${mouseSetup} const leaf = () => position.x; function read() { return (leaf as () => number)(); }`, '((read as () => number)())', '42');
pair('resize-parameter-alias', '@solid-primitives/resize-observer', `import { createWindowSize } from '@solid-primitives/resize-observer';`,
  `const size = createWindowSize(); h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };
   function width(key: 'width') { return size[key]; } const selected = width;`, "selected('width')", '640', true);
pair('set-return-chain', '@solid-primitives/set', `import { ReactiveSet } from '@solid-primitives/set';`,
  `const state = new ReactiveSet<number>(); h.update = () => { state.add(2); flush(); };
   function count() { return state.size; } function read() { return count(); }`, 'read()', '1');
pair('five-helpers-depth', '@solid-primitives/mouse', mouse,
  `${mouseSetup} function one() { return position.x; } function two() { return one(); }
   function three() { return two(); } function four() { return three(); } function five() { return four(); }`, 'five()', '42');
for (const [id, helpers, initializer, display, desired] of [
  ['explicit-outer', `const leaf = () => position.x; function read() { return leaf(); }`, 'untrack(read)', 'frozen', '0'],
  ['explicit-inner', `const leaf = () => position.x; function read() { return untrack(leaf); }`, 'read()', 'frozen', '0'],
  ['discarded-return', `const leaf = () => position.x; function read() { return (leaf(), 9); }`, 'read()', 'frozen', '9'],
  ['discarded-argument', `const leaf = () => position.x; function ignore(_value: number) { return { x: 9 }; } function read() { return ignore(leaf()).x; }`, 'read()', 'frozen', '9'],
  ['branch-return', `function read() { if (true) return 9; return position.x; }`, 'read()', 'frozen', '9'],
  ['explicit-mutable-alias', `const leaf = () => position.x; let selected = leaf;`, 'untrack(selected)', 'frozen', '0'],
  ['discard-before-deferred', `const leaf = () => position.x; function read() { return leaf(); } read();`, '() => read()', 'frozen()', '42'],
  ['different-return-path', `const leaf = () => position.x; function read() { return leaf(); } function other() { return { x: 9 }.x; } untrack(read);`, 'other()', 'frozen', '9'],
]) add(id + '-control', '@solid-primitives/mouse', mouse, `${mouseSetup} ${helpers} const frozen = ${initializer};`, display, desired, false,
  { family: 'local-return-path-control', role: 'control', expectedIssue: false, rules: [] });
for (const [id, setup, code] of [
  ['wrong-key', `function read(key: 'x') { return position[key]; } const frozen = read('absent');`, 2345],
  ['function-assignment', `function read() { return position.x; } [read] = [() => 9]; const frozen = read();`, 2630],
]) add(id + '-type-exclusion', '@solid-primitives/mouse', mouse, `${mouseSetup} ${setup}`, 'frozen', '9', false,
  { family: 'published-type-exclusion', role: 'type-exclusion', expectedIssue: false, rules: [], expectedTypingCode: code });
export default cases;
