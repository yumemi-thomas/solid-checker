// New population authored after the v7 detector freeze; expectations are scoring only.
import './snapshot-feedback-v7.mjs';
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname), roots = read(resolve(repo, 'rust/target/primitives-checkpoint/run-browser.json')).results,
  cases = [], prelude = `import { flush, untrack } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function observe(desired, resize = false) {
  return async page => {
    if (resize) await page.setViewportSize({ width: 640, height: 900 });
    await page.evaluate(() => { const h = globalThis.__experiment; h.attempt('update', h.update); }); await page.waitForTimeout(120);
    await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = {
      desired, actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; }, desired);
  };
}
function add(id, name, imports, setup, jsx, desired, resize, provenance) {
  cases.push({ id: 'snapshot-expression-' + id, package: name,
    app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), roots.find(row => row.package === name).retainedArtifacts.projectDir),
    source: `${prelude}\n${imports}\nfunction App() { ${setup} return ${jsx}; }\nh.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
    flow: observe(desired, resize), provenance });
}
function pair(id, name, imports, setup, jsx, desired, resize = false) {
  for (const bad of [true, false]) add(id + (bad ? '-target' : '-control'), name, imports, setup(bad), jsx(bad), desired, resize,
    { pair: id, role: bad ? 'target' : 'control', expectedIssue: bad, family: 'initializer-snapshot',
      rules: ['strict-read-untracked'], codes: ['STRICT_READ_UNTRACKED', 'SOURCE_CLASS_SNAPSHOT_FLOW', 'OBSERVED_GETTER_SNAPSHOT_FLOW', 'OBSERVED_PACKAGE_SNAPSHOT_FLOW'] });
}
const mouse = `import { createMousePosition } from '@solid-primitives/mouse';`, mouseSetup = `const position = createMousePosition(window, { touch: false });
  h.update = () => { window.dispatchEvent(new MouseEvent('mousemove', { clientX: 42, clientY: 5 })); flush(); };`,
  dimensions = `import { createWindowSize } from '@solid-primitives/resize-observer';`, sizeSetup = `const size = createWindowSize();
  h.update = () => { window.dispatchEvent(new Event('resize')); flush(); };`;
for (const [id, expression, desired] of [['computed-property', "position['x']", '42'],
  ['dynamic-property', 'position[member()]', '42'], ['conditional', "position['x'] > 10 ? 'far' : 'near'", 'far']])
  pair(id, '@solid-primitives/mouse', mouse,
    bad => `${mouseSetup} const member = () => 'x' as const; ${bad ? `const frozen = ${expression};` : ''}`,
    bad => `<p id='value'>{${bad ? 'frozen' : expression}}</p>`, desired);
pair('array', '@solid-primitives/mouse', mouse,
  bad => `${mouseSetup} ${bad ? 'const frozen = [position.x, position.y];' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen.join(",")' : '[position.x, position.y].join(",")'}}</p>`, '42,5');
pair('object', '@solid-primitives/mouse', mouse,
  bad => `${mouseSetup} ${bad ? "const frozen = { x: position['x'], y: position.y };" : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen.x + frozen.y' : "position['x'] + position.y"}}</p>`, '47');
pair('arithmetic', '@solid-primitives/resize-observer', dimensions,
  bad => `${sizeSetup} ${bad ? "const frozen = size['width'] * 2;" : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : "size['width'] * 2"}}</p>`, '1280', true);
pair('template', '@solid-primitives/resize-observer', dimensions,
  bad => `${sizeSetup} ${bad ? 'const frozen = `${size["width"]}/${size.height}`;' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : '`${size["width"]}/${size.height}`'}}</p>`, '640/900', true);
pair('bounds', '@solid-primitives/bounds', `import { createElementBounds } from '@solid-primitives/bounds';`,
  bad => `const element = document.createElement('div'); element.style.cssText = 'width:20px;height:20px'; document.body.append(element);
    const bounds = createElementBounds(element, { trackMutation: false, trackResize: false });
    h.update = () => { element.style.width = '40px'; document.body.dispatchEvent(new Event('scroll')); flush(); };
    ${bad ? "const frozen = bounds['width'];" : ''}`,
  bad => `<p id='value'>{String(${bad ? 'frozen' : "bounds['width']"})}</p>`, '40');
pair('active-element', '@solid-primitives/active-element', `import { createActiveElement } from '@solid-primitives/active-element';`,
  bad => `const active = createActiveElement(); h.update = () => { document.getElementById('focus-target')!.focus(); flush(); };
    ${bad ? "const frozen = active()?.id ?? 'none';" : ''}`,
  bad => `<><button id='focus-target'>focus</button><p id='value'>{${bad ? 'frozen' : "active()?.id ?? 'none'"}}</p></>`, 'focus-target');
// A local helper separates the guard's source location from the displayed initializer.
pair('named-helper', '@solid-primitives/mouse', mouse,
  bad => `${mouseSetup} function readX() { return position['x']; } ${bad ? 'const frozen = readX();' : ''}`,
  bad => `<p id='value'>{${bad ? 'frozen' : 'readX()'}}</p>`, '42');
const controls = [
  ['discarded-comma', mouse, `${mouseSetup} const frozen = (position['x'], 9);`, 'frozen', '9'],
  ['ignored-argument', mouse, `${mouseSetup} function ignore(_value: number) { return 9; } const frozen = ignore(position['x']);`, 'frozen', '9'],
  ['namespace-snapshot', mouse + ` import * as core from 'solid-js';`, `${mouseSetup} const frozen = core.untrack(() => position['x'] + position.y);`, 'frozen', '0'],
  ['const-alias-snapshot', mouse, `${mouseSetup} const snapshot = untrack; const frozen = snapshot(() => position['x']);`, 'frozen', '0'],
  ['snapshot-in-helper', mouse, `${mouseSetup} function identity(value: number) { return value; } const frozen = identity(untrack(() => position['x']));`, 'frozen', '0'],
  ['explicit-object', mouse, `${mouseSetup} const frozen = untrack(() => ({ x: position['x'] }));`, 'frozen.x', '0'],
  ['unexecuted-branch', mouse, `${mouseSetup} const frozen = false ? position['x'] : 9;`, 'frozen', '9'],
  ['deferred-read', mouse, `${mouseSetup} const frozen = () => position['x'];`, 'frozen()', '42'],
];
for (const [id, imports, setup, display, desired] of controls) add(id + '-control', '@solid-primitives/mouse', imports, setup,
  `<p id='value'>{${display}}</p>`, desired, false, { family: 'value-flow-control', role: 'control', expectedIssue: false, rules: [] });
for (const [id, name, imports, setup, code] of [
  ['unknown-property', '@solid-primitives/mouse', mouse, "const position = createMousePosition(window); const frozen = position['missing'];", 7053],
  ['readonly-bounds', '@solid-primitives/bounds', `import { createElementBounds } from '@solid-primitives/bounds';`,
    'const bounds = createElementBounds(document.body); bounds.width = 12;', 2540],
]) add(id + '-typing-exclusion', name, imports, setup, '<p>typing</p>', 'typing', false,
  { family: 'type-exclusion', role: 'control', expectedIssue: false, rules: [], expectedTypingCode: code });
export default cases;
