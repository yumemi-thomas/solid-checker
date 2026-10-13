// Includes semantic write failures that the actual published typings allow.
const cases = [], prelude = `import { createSignal, createMemo, flush } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
function add(id, pkg, imports, setup, jsx, action, desired, provenance) {
  cases.push({ id: 'getter-' + id, package: pkg,
    source: `${prelude}\n${imports}\nfunction App() { ${setup}\nh.read = () => ${provenance.directRead ?? 'null'}; h.start = () => h.attempt('change', () => { ${action}; flush(); }); return ${jsx}; } h.dispose = render(() => <App />, document.getElementById('root')!);`,
    provenance: { family: 'source-getter-path', role: 'control', expectedIssue: false, expectedSourceNote: false, ...provenance },
    flow: async (page, step) => { await step('change', () => page.evaluate(() => globalThis.__experiment.start())); await page.waitForTimeout(100);
      await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = { desired: String(desired), actual: document.getElementById('value')?.textContent }; h.values.direct = h.read(); }, desired);
      await step('dispose', () => page.evaluate(() => { const h = globalThis.__experiment; h.dispose(); h.disposals++; })); } });
}
const storeImport = `import { createStaticStore, createDerivedStaticStore } from '@solid-primitives/static-store';`;
for (const bad of [true, false]) add('static-write-' + (bad ? 'target' : 'control'), '@solid-primitives/static-store', storeImport,
  `const [state, setState] = createStaticStore({ count: 0 });`, `<p id='value'>{state.count}</p>`, bad ? 'state.count = 1' : `setState('count', 1)`, bad ? 0 : 1,
  { role: bad ? 'target' : 'control', expectedIssue: bad, expectedSourceNote: bad, expectedSourceCode: 'SOURCE_GETTER_WRITE_CANDIDATE', expectedException: bad });
for (const bad of [true, false]) add('derived-write-' + (bad ? 'target' : 'control'), '@solid-primitives/static-store', storeImport,
  `const [count, setCount] = createSignal(0); const state = createDerivedStaticStore(() => ({ count: count() }));`, `<p id='value'>{state.count}</p>`, bad ? 'state.count = 1' : `setCount(1)`, bad ? 0 : 1,
  { role: bad ? 'target' : 'control', expectedIssue: bad, expectedSourceNote: bad, expectedSourceCode: 'SOURCE_GETTER_WRITE_CANDIDATE', expectedException: bad });
for (const [id, pkg, api] of [['readonly-bounds', 'bounds', 'createElementBounds'], ['readonly-size', 'resize-observer', 'createElementSize']])
  add(id + '-typing-candidate', '@solid-primitives/' + pkg, `import { ${api} } from '@solid-primitives/${pkg}';`,
    `const box = document.createElement('div'); box.style.width = '100px'; document.body.appendChild(box); const state = ${api}(box);`, `<p id='value'>{state.width}</p>`, `state.width = 99`, 100,
    { category: 'typing-candidate', expectedTypingCode: 2540 });
add('plain-bounds-write-control', '@solid-primitives/bounds', `import { getElementBounds } from '@solid-primitives/bounds';`,
  `const box = document.createElement('div'); document.body.appendChild(box); const state = getElementBounds(box);`, `<p id='value'>plain</p>`, 'state.width = 99', 'plain', { directRead: 'state.width', desiredDirect: 99 });
add('descriptor-replaced-control', '@solid-primitives/static-store', storeImport,
  `const [state] = createStaticStore({ count: 0 }); Object.defineProperty(state, 'count', { value: 99, writable: true });`, `<p id='value'>replaced</p>`, 'state.count = 1', 'replaced',
  { directRead: 'state.count', desiredDirect: 1 });
for (const bad of [true, false]) add('namespace-snapshot-' + (bad ? 'target' : 'control'), '@solid-primitives/static-store', `import * as Store from '@solid-primitives/static-store';`,
  `const [state, setState] = Store.createStaticStore({ count: 0 }); ${bad ? 'const snapshot = state.count;' : ''}`, `<p id='value'>{${bad ? 'snapshot' : 'state.count'}}</p>`, `setState('count', 1)`, 1,
  { role: bad ? 'target' : 'control', expectedIssue: bad, expectedSourceNote: bad, expectedSourceCode: 'SOURCE_GETTER_SNAPSHOT_FLOW', expectedBehaviorFailure: bad });
add('tracked-memo-control', '@solid-primitives/static-store', storeImport,
  `const [state, setState] = createStaticStore({ count: 0 }); const view = createMemo((() => { const value = state.count; return <p id='value'>{value}</p>; }));`, `<>{view()}</>`, `setState('count', 1)`, 1, {});
add('aliased-seed-target', '@solid-primitives/static-store', storeImport,
  `const seed = { count: 0 }; const [state, setState] = createStaticStore(seed); const snapshot = state.count;`, `<p id='value'>{snapshot}</p>`, `setState('count', 1)`, 1,
  { role: 'target', expectedIssue: true, expectedBehaviorFailure: true, gap: 'consumer local argument shape is unresolved' });
export default cases;
