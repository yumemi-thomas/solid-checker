// Paired, declared mutations. Observed failures are finite behavior evidence;
// snapshot intent and resource lifetime are specified by each test, not guessed.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { hash } from "./catalog.mjs";
const cases = [];
const prelude = `import { createSignal, createMemo, createEffect, flush, untrack } from 'solid-js'; import { render } from '@solidjs/web';
const h = (globalThis as any).__experiment;`;
const mount = `h.dispose = render(() => <App />, document.getElementById('root')!);`;
const dispose = async page => page.evaluate(() => { const h = (globalThis).__experiment; h.dispose?.(); h.disposals++; });
function pair(id, packageName, build, flow, provenance = {}) {
  for (const role of ['target', 'control']) cases.push({ id: `breadth-${id}-${role}`, package: packageName,
    ...provenance.install, source: build(role === 'target'), flow: async (page, step) => { await flow(page, step, role === 'target'); },
    provenance: { pair: id, role, expectedIssue: role === 'target', authoredMutation: true, ...provenance, install: undefined } });
}
const bodySource = (imports, body, expression) => `${prelude}\n${imports}\nfunction App() { ${body}\nreturn <p id='value'>{String(${expression})}</p>; }\n${mount}`;
function updatePair(id, packageName, imports, setup, expression, update, desired, { initial = null, action = null, install, ...provenance } = {}) {
  pair(id, packageName, bad => bodySource(imports, `${setup}\nh.update = () => { ${update} };\n${bad ? `const snapshot = ${expression};` : ''}`, bad ? 'snapshot' : expression),
    async (page, step, bad) => {
      if (initial !== null) await page.waitForFunction(value => document.getElementById('value')?.textContent === String(value), initial);
      await step('change-input', async () => { if (action) await action(page); else await page.evaluate(() => globalThis.__experiment.update()); });
      if (!bad) await page.waitForFunction(value => document.getElementById('value')?.textContent === String(value), desired);
      // The input change has occurred in both twins. Keep the actual value,
      // including unexpected mutant success; the validator determines outcome.
      await page.waitForTimeout(100);
      await page.evaluate(desired => { const h = globalThis.__experiment; h.values.behavior = { desired: String(desired), actual: document.getElementById('value')?.textContent }; }, desired);
      await step('dispose', () => dispose(page));
    }, { ...provenance, expectation: 'display follows changed input', desired, install });
}
updatePair('static-store-member', '@solid-primitives/static-store', `import { createStaticStore } from '@solid-primitives/static-store';`,
  `const [state, setState] = createStaticStore({ count: 0 });`, 'state.count', `setState('count', 1); flush();`, 1, { initial: 0, family: 'reactive-object' });
updatePair('derived-store-member', '@solid-primitives/static-store', `import { createDerivedStaticStore } from '@solid-primitives/static-store';`,
  `const [count, setCount] = createSignal(0); const state = createDerivedStaticStore(() => ({ count: count() }));`, 'state.count', `setCount(1); flush();`, 1, { initial: 0, family: 'reactive-object' });
updatePair('map-get', '@solid-primitives/map', `import { ReactiveMap } from '@solid-primitives/map';`,
  `const map = new ReactiveMap<string, number>([['key', 0]]);`, `map.get('key')`, `map.set('key', 1); flush();`, 1, { initial: 0, family: 'collection-method' });
updatePair('set-has', '@solid-primitives/set', `import { ReactiveSet } from '@solid-primitives/set';`,
  `const set = new ReactiveSet<number>();`, `set.has(1)`, `set.add(1); flush();`, true, { initial: false, family: 'collection-method' });
updatePair('combined-props', '@solid-primitives/props', `import { combineProps } from '@solid-primitives/props';`,
  `const [count, setCount] = createSignal(0); const props = combineProps({ get count() { return count(); } });`, 'props.count', `setCount(1); flush();`, 1, { initial: 0, family: 'proxy-props' });
const element = `const box = document.createElement('div'); box.style.cssText = 'width:100px;height:20px'; document.body.appendChild(box);`;
for (const [id, pkg, name] of [['element-size', 'resize-observer', 'createElementSize'], ['element-bounds', 'bounds', 'createElementBounds']])
  updatePair(id, `@solid-primitives/${pkg}`, `import { ${name} } from '@solid-primitives/${pkg}';`,
    `${element} const state = ${name}(box);`, 'state.width', `box.style.width = '200px';`, 200, { initial: 100, family: 'dom-observer' });
updatePair('mouse-object', '@solid-primitives/mouse', `import { createMousePosition } from '@solid-primitives/mouse';`,
  `const state = createMousePosition(window, { touch: false });`, 'state.x', `window.dispatchEvent(new MouseEvent('mousemove', { clientX: 120, clientY: 30 })); flush();`, 120,
  { initial: 0, family: 'event-state' });
updatePair('pointer-accessor', '@solid-primitives/pointer', `import { createPointerPosition } from '@solid-primitives/pointer';`,
  `const state = createPointerPosition({ target: window });`, 'state().x', `window.dispatchEvent(new PointerEvent('pointerenter', { pointerId: 1, pointerType: 'mouse', clientX: 120, clientY: 30 })); flush();`, 120,
  { family: 'event-state' });
updatePair('media-options', '@solid-primitives/media', `import { createMediaQuery } from '@solid-primitives/media';`,
  `const state = createMediaQuery('(min-width: 600px)');`, 'state()', '', false,
  { initial: true, action: page => page.setViewportSize({ width: 500, height: 700 }), family: 'accessor' });

pair('reactive-listener-target', '@solid-primitives/event-listener', bad => bodySource(`import { createEventListener } from '@solid-primitives/event-listener';`,
  `const first = new EventTarget(), next = new EventTarget(); const [target, setTarget] = createSignal(first); const [count, setCount] = createSignal(0);
createEventListener(${bad ? 'target()' : 'target'}, 'click', () => setCount(value => value + 1));
h.update = () => { setTarget(next); flush(); next.dispatchEvent(new Event('click')); flush(); };`, 'count()'),
  async (page, step) => { await step('replace-target-and-dispatch', () => page.evaluate(() => globalThis.__experiment.update())); await page.waitForTimeout(50); await page.evaluate(() => { const h = globalThis.__experiment; h.values.behavior = { desired: '1', actual: document.getElementById('value').textContent }; }); await dispose(page); },
  { family: 'reactive-options', expectation: 'listener follows replaced target', desired: 1 });

for (const name of ['debounce', 'throttle']) pair(`${name}-event-lifetime`, '@solid-primitives/scheduled', bad => bodySource(`import { ${name} } from '@solid-primitives/scheduled';`,
  `const callback = () => { h.values.calls = (h.values.calls ?? 0) + 1; }; ${bad ? '' : `const scheduled = ${name}(callback, 40);`}
h.start = () => { ${bad ? `const scheduled = ${name}(callback, 40);` : ''} scheduled(); };`, '0'),
  async (page, step) => { await step('schedule-and-dispose', () => page.evaluate(() => { const h = globalThis.__experiment; h.start(); h.dispose(); h.disposals++; })); await page.waitForTimeout(100); await page.evaluate(() => { const h = globalThis.__experiment; h.values.behavior = { desired: '0', actual: String(h.values.calls ?? 0) }; }); },
  { family: 'delayed-callback-lifetime', expectation: 'component callback cancelled on disposal', desired: 0 });

pair('listener-event-lifetime', '@solid-primitives/event-listener', bad => bodySource(`import { makeEventListener } from '@solid-primitives/event-listener';`,
  `const target = new EventTarget(); const callback = () => { h.values.calls = (h.values.calls ?? 0) + 1; };
${bad ? '' : `makeEventListener(target, 'click', callback);`} h.start = () => { ${bad ? `makeEventListener(target, 'click', callback);` : ''} }; h.dispatch = () => target.dispatchEvent(new Event('click'));`, '0'),
  async (page, step) => { await step('register-dispose-and-dispatch', () => page.evaluate(() => { const h = globalThis.__experiment; h.start(); h.dispose(); h.disposals++; h.dispatch(); h.values.behavior = { desired: '0', actual: String(h.values.calls ?? 0) }; })); },
  { family: 'listener-lifetime', expectation: 'component listener removed on disposal', desired: 0 });

pair('abort-after-await', '@solid-primitives/async', bad => bodySource(`import { createAbortable } from '@solid-primitives/async';`,
  `${bad ? '' : 'const [signal] = createAbortable();'}
h.start = async () => { await Promise.resolve(); ${bad ? 'const [signal] = createAbortable();' : ''} h.active = signal(); };`, '0'),
  async (page, step) => { await step('start-request', () => page.evaluate(() => globalThis.__experiment.start())); await step('dispose-request', () => page.evaluate(() => { const h = globalThis.__experiment; h.dispose(); h.disposals++; h.values.behavior = { desired: 'true', actual: String(h.active.aborted) }; })); },
  { family: 'async-owner-loss', expectation: 'request aborts on component disposal', desired: true });

pair('memo-callback-write', '@solid-primitives/memo', bad => bodySource(`import { createLazyMemo } from '@solid-primitives/memo';`,
  `const [count, setCount] = createSignal(0); const state = createLazyMemo(() => { ${bad ? `h.attempt('callback-write', () => setCount(1));` : ''} return count(); });`, 'state()'),
  async page => dispose(page), { family: 'callback-phase', expectation: 'tracked callback must not write its source', diagnosticTarget: 'REACTIVE_WRITE_IN_OWNED_SCOPE' });
pair('event-bus-callback-phase', '@solid-primitives/event-bus', bad => bodySource(`import { createEventBus } from '@solid-primitives/event-bus';`,
  `const [count, setCount] = createSignal(0); const bus = createEventBus<number>(); bus.listen(value => setCount(value));
${bad ? `const trigger = createMemo(() => { h.attempt('callback-write', () => bus.emit(1)); return 0; }); createEffect(trigger, () => {});` : ''}
h.update = () => { ${bad ? '' : 'bus.emit(1);'} flush(); };`, 'count()'),
  async (page, step) => { await step('emit', () => page.evaluate(() => globalThis.__experiment.update())); await page.evaluate(() => { const h = globalThis.__experiment; h.values.behavior = { desired: '1', actual: document.getElementById('value').textContent }; }); await dispose(page); },
  { family: 'callback-phase', expectation: 'imperative emission writes outside tracked compute', diagnosticTarget: 'REACTIVE_WRITE_IN_OWNED_SCOPE', desired: 1 });

pair('router-hook-event', '@solidjs/router', bad => `${prelude}
import { createRouter, useNavigate } from '@solidjs/router';
function App() { ${bad ? '' : 'const navigate = useNavigate();'} return <button id='navigate' onClick={() => h.attempt('navigation', () => ${bad ? 'useNavigate()' : 'navigate'}('/next'))}>navigate</button>; }
const Router = createRouter({ routes: [{ path: '/', component: App }, { path: '/next', component: () => <p>next</p> }] });
h.dispose = render(() => <Router />, document.getElementById('root')!);`,
  async (page, step) => { await step('click-navigation', () => page.locator('#navigate').click()); await page.waitForTimeout(50); await page.evaluate(() => { const h = globalThis.__experiment; h.values.behavior = { desired: '/next', actual: location.pathname }; }); await dispose(page); },
  { install: { app: 'helge-dev' }, family: 'context', expectation: 'navigation handler reaches next route', desired: '/next' });
pair('dialog-context', '@corvu-next/dialog', bad => `${prelude}
import Dialog from '@corvu-next/dialog';
function Child() { const context = Dialog.useContext(); return <p id='value'>{String(context.open())}</p>; }
function App() { return ${bad ? '<Child />' : '<Dialog><Child /></Dialog>'}; }
${mount}`, async page => dispose(page),
  { install: { app: 'sefer' }, family: 'context', expectation: 'dialog hook runs under its provider', exceptionTarget: true });
pair('dialog-trigger-provider', '@corvu-next/dialog', bad => `${prelude}
import { Root, Trigger } from '@corvu-next/dialog'; function App() { return ${bad ? '<Trigger>open</Trigger>' : '<Root><Trigger>open</Trigger></Root>'}; } ${mount}`,
  async page => dispose(page), { install: { app: 'sefer' }, family: 'component-provider', expectation: 'trigger runs under its provider', exceptionTarget: true });

pair('meta-title-snapshot', '@solidjs/meta', bad => `${prelude}
import { Title } from '@solidjs/meta'; function App() { const [count, setCount] = createSignal(0); h.update = () => { setCount(1); flush(); }; ${bad ? 'const snapshot = count();' : ''} return <Title>{${bad ? 'snapshot' : 'count()'}}</Title>; } ${mount}`,
  async (page, step, bad) => { await step('change-title-input', () => page.evaluate(() => globalThis.__experiment.update())); if (!bad) await page.waitForFunction(() => document.title === '1'); await page.waitForTimeout(50); await page.evaluate(() => { globalThis.__experiment.values.behavior = { desired: '1', actual: document.title }; }); await dispose(page); },
  { install: { app: 'helge-dev' }, family: 'head-component', expectation: 'document title follows input', desired: 1 });

pair('query-data-destructure', '@tanstack/solid-query', bad => `${prelude}
import { QueryClient, QueryClientProvider, useQuery } from '@tanstack/solid-query';
const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
function Child() { const query = useQuery(() => ({ queryKey: ['local'], queryFn: async () => 0, initialData: 0 })); ${bad ? 'const { data } = query;' : ''}
h.update = () => client.setQueryData(['local'], 1); return <p id='value'>{${bad ? 'data' : 'query.data'}}</p>; }
function App() { return <QueryClientProvider client={client}><Child /></QueryClientProvider>; } ${mount}`,
  async (page, step, bad) => { await page.locator('#value').waitFor(); await step('change-query-data', () => page.evaluate(() => globalThis.__experiment.update())); if (!bad) await page.waitForFunction(() => document.getElementById('value')?.textContent === '1'); await page.waitForTimeout(50); await page.evaluate(() => { const h = globalThis.__experiment; h.values.behavior = { desired: '1', actual: document.getElementById('value').textContent }; }); await dispose(page); },
  { install: { app: 'spotify-desk-thing' }, family: 'query-result', expectation: 'query result display follows cache update', desired: 1, olderRuntime: true });

// Existing application sources supply two integration mutations. Only copied
// source is edited; exact text and its input/output digests are retained.
for (const variant of ['navigation', 'modal']) for (const role of ['target', 'control']) cases.push({ id: `breadth-app-${variant}-${role}`, app: 'helge-dev', appCopy: true,
  packages: ['@solidjs/router', '@solidjs/meta', 'solid-icons'], source: null,
  provenance: { pair: `app-${variant}`, role, expectedIssue: role === 'target', authoredMutation: true, family: 'actual-app', expectation: variant === 'navigation' ? 'mobile menu closes on route change' : 'modal opens after click' },
  patchCopy: role === 'target' ? root => {
    const path = join(root, variant === 'navigation' ? 'src/components/NavBar.tsx' : 'src/components/Modal.tsx'), original = readFileSync(path, 'utf8');
    const before = variant === 'navigation' ? '\tcreateEffect(\n\t\t() => location.pathname,' : '\treturn (\n\t\t<Show when={modalOpened()}>';
    const after = variant === 'navigation' ? '\tconst snapshotPath = location.pathname;\n\tcreateEffect(\n\t\t() => snapshotPath,' : '\tconst snapshotOpened = modalOpened();\n\treturn (\n\t\t<Show when={snapshotOpened}>';
    assert(original.includes(before)); const changed = original.replace(before, after); writeFileSync(path, changed);
    return { path, originalSha256: hash(original), mutatedSha256: hash(changed), before, after };
  } : null,
  flow: async (page, step) => {
    if (variant === 'navigation') { await page.setViewportSize({ width: 390, height: 844 }); await page.getByRole('button', { name: 'menu-burger-button' }).click(); await page.locator('.responsiveButtons').getByRole('link', { name: 'About', exact: true }).click(); await page.waitForURL('**/about'); await page.waitForTimeout(100); await page.evaluate(() => { globalThis.__experiment.values.behavior = { desired: 'false', actual: String(!!document.querySelector('.NavBar.open')) }; }); }
    else { await step('open-modal', () => page.locator('.icons [role=button]').click()); await page.waitForTimeout(100); await page.evaluate(() => { globalThis.__experiment.values.behavior = { desired: 'true', actual: String(!!document.querySelector('.modal')) }; }); }
    await dispose(page);
  } });

// Intent controls prevent the behavior oracle from equating every snapshot or
// background resource with a defect. They assert different, explicit lifetimes.
cases.push({ id: 'breadth-intentional-snapshot-control', package: '@solid-primitives/static-store',
  source: bodySource(`import { createStaticStore } from '@solid-primitives/static-store';`, `const [state, setState] = createStaticStore({ count: 0 }); const snapshot = state.count; h.update = () => { setState('count', 1); flush(); };`, 'snapshot'),
  provenance: { pair: 'intentional-snapshot', role: 'control', expectedIssue: false, family: 'intent-control', expectation: 'display intentionally retains initial snapshot' },
  flow: async page => { await page.evaluate(() => { const h = globalThis.__experiment; h.update(); h.values.behavior = { desired: '0', actual: document.getElementById('value').textContent }; }); await dispose(page); } });
cases.push({ id: 'breadth-background-scheduled-control', package: '@solid-primitives/scheduled',
  source: `${prelude} import { debounce } from '@solid-primitives/scheduled'; const scheduled = debounce(() => { h.values.calls = (h.values.calls ?? 0) + 1; }, 40); function App() { h.start = () => scheduled(); return <p>background</p>; } ${mount}`,
  provenance: { pair: 'background-scheduled', role: 'control', expectedIssue: false, family: 'intent-control', expectation: 'background callback intentionally outlives component' },
  flow: async page => { await page.evaluate(() => { const h = globalThis.__experiment; h.start(); h.dispose(); h.disposals++; }); await page.waitForTimeout(100); await page.evaluate(() => { const h = globalThis.__experiment; h.values.behavior = { desired: '1', actual: String(h.values.calls ?? 0) }; }); } });
export default cases;
