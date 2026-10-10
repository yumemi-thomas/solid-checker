// Probe additional automatic channels and exact-code intent counterexamples.
// Labels separate execution mistakes, advisory patterns and typing candidates.
import breadth from "./breadth-cases.mjs";
const cases = [];
const imports = `import { createSignal, createMemo, createEffect, createErrorBoundary, createStore, flush } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
const dispose = async page => page.evaluate(() => { const h = globalThis.__experiment; h.dispose(); h.disposals++; });
function pair(id, install, make, flow, provenance) {
  for (const role of ['target', 'control']) cases.push({ id: `automatic-${id}-${role}`, ...install,
    source: make(role === 'target'), flow,
    provenance: { pair: id, role, profile: 'channels', expectedIssue: role === 'target', ...provenance } });
}
const component = (extra, setup, jsx) => `${imports} ${extra} function App() { ${setup} return ${jsx}; } h.dispose = render(() => <App />, document.getElementById('root')!);`;
const mounted = async (page, step) => { await step('settle', () => page.waitForTimeout(80)); await page.evaluate(() => { globalThis.__experiment.values.dom = document.getElementById('root')?.textContent; }); await dispose(page); };
pair('autofocus-cleanup', { package: '@solid-primitives/focus' }, bad => component(`import { autofocus } from '@solid-primitives/focus';`,
  `${bad ? '' : 'const ref = autofocus();'} h.start = () => { ${bad ? `autofocus()(document.getElementById('focus')!);` : ''} };`,
  `<button id='focus' autofocus ${bad ? '' : 'ref={ref}'} onClick={() => h.start()}>focus</button>`),
  async (page, step) => { await step('focus-event', () => page.locator('#focus').click()); await mounted(page, step); },
  { category: 'execution', expectedCode: 'SETTLED_CLEANUP_UNOWNED' });
cases.push({ id: 'automatic-autofocus-disabled-control', package: '@solid-primitives/focus',
  source: component(`import { autofocus } from '@solid-primitives/focus';`, `h.start = () => autofocus()(document.getElementById('focus')!);`, `<button id='focus' onClick={() => h.start()}>focus</button>`),
  flow: async (page, step) => { await step('focus-event', () => page.locator('#focus').click()); await mounted(page, step); },
  provenance: { pair: 'autofocus-disabled', role: 'control', profile: 'channels', expectedIssue: false, category: 'execution' } });
pair('boundary-owner', { package: '@solid-primitives/memo' }, bad => component(`import { createLazyMemo } from '@solid-primitives/memo';`,
  `const state = createLazyMemo(() => 1); ${bad ? '' : 'const boundary = createErrorBoundary(() => state(), () => 0);'} h.start = () => h.attempt('boundary', () => ${bad ? 'createErrorBoundary(() => state(), () => 0)' : 'boundary'});`,
  `<button id='start' onClick={() => h.start()}>start</button>`),
  async (page, step) => { await step('boundary-event', () => page.locator('#start').click()); await mounted(page, step); },
  { category: 'execution', expectedCode: 'NO_OWNER_BOUNDARY' });
pair('effect-bus-cycle', { package: '@solid-primitives/event-bus' }, bad => component(`import { createEventBus } from '@solid-primitives/event-bus';`,
  `const [count, setCount] = createSignal(${bad ? 0 : 1}, { name: 'count' }); const bus = createEventBus<number>(); bus.listen(setCount);
createEffect(count, value => { if (value < 1) bus.emit(1); }, { name: 'normalize-bus' });`, `<p>{count()}</p>`), mounted,
  { category: 'advisory', expectedCode: 'EFFECT_WRITES_OWN_SOURCE' });
pair('effect-listener-cycle', { package: '@solid-primitives/event-listener' }, bad => component(`import { makeEventListener } from '@solid-primitives/event-listener';`,
  `const [count, setCount] = createSignal(${bad ? 0 : 1}, { name: 'count' }); const target = new EventTarget(); makeEventListener(target, 'change', () => setCount(1));
createEffect(count, value => { if (value < 1) target.dispatchEvent(new Event('change')); }, { name: 'normalize-listener' });`, `<p>{count()}</p>`), mounted,
  { category: 'advisory', expectedCode: 'EFFECT_WRITES_OWN_SOURCE' });
pair('unstable-lazy-memo', { package: '@solid-primitives/memo' }, bad => component(`import { createLazyMemo } from '@solid-primitives/memo';`,
  `const [tick, setTick] = createSignal(0); const value = createLazyMemo(() => ${bad ? '({ parity: tick() % 2 })' : 'tick() % 2'}, ${bad ? '{ parity: 0 }' : '0'}, { name: 'parity' });
h.start = () => { for (let i = 0; i < 6; i++) { setTick(value => value + 2); flush(); } };`, `<button id='start' onClick={() => h.start()}>{${bad ? 'value().parity' : 'value()'}}</button>`),
  async (page, step) => { await step('update-same-output', () => page.locator('#start').click()); await mounted(page, step); },
  { category: 'advisory', expectedCode: 'UNSTABLE_MEMO_OUTPUT' });
pair('async-store-setter', { app: 'helge-dev', packages: ['solid-js'] }, bad => component('',
  `const [state, setState] = createStore({ count: 0 }); h.start = () => h.attempt('setter', () => setState(${bad ? 'async draft => { await Promise.resolve(); draft.count = 1; }' : 'draft => { draft.count = 1; }'}));`,
  `<button id='start' onClick={() => h.start()}>{state.count}</button>`),
  async (page, step) => { await step('store-event', () => page.locator('#start').click()); await mounted(page, step); },
  { category: 'typing-candidate', expectedCode: 'ASYNC_STORE_SETTER' });

// Each pair uses byte-identical application code and identical actions. Only
// the declared desired behavior differs. A runtime observation cannot recover
// an intention that is absent from the executable program.
for (const [pairName, targetId, desiredTarget, desiredControl] of [
  ['snapshot-intent', 'breadth-static-store-member-target', '1', '0'],
  ['background-intent', 'breadth-debounce-event-lifetime-target', '0', '1'],
]) {
  const original = breadth.find(entry => entry.id === targetId);
  for (const [role, desired] of [['target', desiredTarget], ['control', desiredControl]]) cases.push({
    id: `automatic-${pairName}-${role}`, package: original.package, source: original.source,
    flow: async (page, step) => { await original.flow(page, step); await page.evaluate(desired => { globalThis.__experiment.values.behavior.desired = desired; }, desired); },
    provenance: { pair: pairName, role, profile: 'intent', expectedIssue: role === 'target', category: 'declared-intent', desired },
  });
}
export default cases.filter(entry => !process.env.REVIEWED_MODEL_AUTOMATIC_PROFILE || entry.provenance.profile === process.env.REVIEWED_MODEL_AUTOMATIC_PROFILE);
