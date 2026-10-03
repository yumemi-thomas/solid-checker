import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
const cases = [];
const prelude = `import { onCleanup, createSignal, Show, flush } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;`;
const bridge = new URL('./automatic-lifetime-runtime.mjs', import.meta.url).pathname;
const source = (imports, setup, handler) => `${prelude}\n${imports}\nfunction App() { ${setup} return <button id='start' onClick={${handler}}>start</button>; } h.dispose = render(() => <App />, document.getElementById('root')!);`;
const snapshot = async page => page.evaluate(() => { const h = globalThis.__experiment; h.values.resourceAudit = globalThis.__resourceAudit?.snapshot() ?? null; h.values.automaticState = globalThis.__automaticLifetimes ?? null; });
const end = async page => page.evaluate(async () => { const h = globalThis.__experiment; h.dispose(); h.disposals++; await Promise.resolve(); });
const scheduled = `import { debounce } from '@solid-primitives/scheduled';`;
for (const [id, setup, handler, nativeMiss] of [
  ['sync-event', '', `() => { debounce(() => h.values.calls = 1, 120)(); }`, false],
  ['await-event', '', `async () => { await Promise.resolve(); debounce(() => h.values.calls = 1, 120)(); h.ready = true; }`, false],
  ['catch-finally', '', `async () => { try { await Promise.reject('expected'); } catch (error) { h.values.caught = error; } finally { debounce(() => h.values.calls = 1, 120)(); h.ready = true; } }`, false],
  ['nested-async', `async function later() { await Promise.resolve(); debounce(() => h.values.calls = 1, 120)(); }`, `async () => { await later(); h.ready = true; }`, false],
  ['await-in-arguments', '', `async () => { debounce(() => h.values.calls = 1, await Promise.resolve(120))(); h.ready = true; }`, false],
]) cases.push({ id: `automatic-life-${id}-target`, package: '@solid-primitives/scheduled', source: source(scheduled, setup, handler),
  flow: async (page, step) => { await step('click', () => page.locator('#start').click()); if (id !== 'sync-event') await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); },
  provenance: { role: 'target', expectedIssue: true, nativeMiss, synchronous: id === 'sync-event', expectedKind: 'timeout' } });
cases.push({ id: 'automatic-life-owned-control', package: '@solid-primitives/scheduled', source: source(scheduled, `const scheduled = debounce(() => h.values.calls = 1, 120);`, `() => scheduled()`),
  flow: async page => { await page.locator('#start').click(); await end(page); await page.waitForTimeout(160); await snapshot(page); }, provenance: { role: 'control', expectedIssue: false } });
cases.push({ id: 'automatic-life-manual-control', package: '@solid-primitives/scheduled', source: source(scheduled, `let clear: (() => void) | undefined; onCleanup(() => clear?.());`,
  `async () => { await Promise.resolve(); const fn = debounce(() => h.values.calls = 1, 120); clear = fn.clear; fn(); h.ready = true; }`),
  flow: async page => { await page.locator('#start').click(); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); }, provenance: { role: 'control', expectedIssue: false } });
cases.push({ id: 'automatic-life-background-control', package: '@solid-primitives/scheduled', source: source(`${scheduled} import { background } from ${JSON.stringify(bridge)};`, '',
  `background(async () => { await Promise.resolve(); debounce(() => h.values.calls = 1, 120)(); h.ready = true; })`),
  flow: async page => { await page.locator('#start').click(); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); }, provenance: { role: 'control', expectedIssue: false, intentionalBackground: true } });
cases.push({ id: 'automatic-life-concurrent-control', package: '@solid-primitives/scheduled', source: source(scheduled, `const gate = new Promise<void>(resolve => h.resume = resolve); h.background = () => debounce(() => h.values.calls = 1, 120)();`,
  `async () => { await gate; h.ready = true; }`),
  flow: async page => { await page.locator('#start').click(); await page.evaluate(() => { const h = globalThis.__experiment; h.background(); h.resume(); }); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); }, provenance: { role: 'control', expectedIssue: false, intentionalBackground: true } });
for (const [id, awaitValue] of [
  ['promise', 'Promise.resolve(1)'], ['thenable', `({ then(resolve: (value: number) => void) { h.values.order.push('then'); resolve(1); } })`],
  ['promise-subclass', `new (class extends Promise<number> { then<T = number, U = never>(yes?: ((value: number) => T | PromiseLike<T>) | null, no?: ((reason: any) => U | PromiseLike<U>) | null): Promise<T | U> { h.values.order.push('then'); return super.then(yes, no); } })(resolve => resolve(1))`],
]) cases.push({ id: `automatic-life-order-${id}-control`, package: '@solid-primitives/scheduled', source: source(scheduled, `h.values.order = [];`,
  `async () => { h.values.order.push('start'); await ${awaitValue}; h.values.order.push('resumed'); h.ready = true; }`),
  flow: async (page, step) => { await step('scheduling-order', () => page.evaluate(() => { document.getElementById('start').click(); const h = globalThis.__experiment; h.values.order.push('after-click'); queueMicrotask(() => h.values.order.push('microtask')); Promise.resolve().then(() => h.values.order.push('then-job')); })); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await snapshot(page); },
  provenance: { role: 'control', expectedIssue: false, schedulingControl: true } });
// Component removal ends its own lifetime while the render root stays alive.
cases.push({ id: 'automatic-life-child-continuation-target', package: '@solid-primitives/scheduled',
  source: `${prelude}\n${scheduled}\nfunction Child() { const gate = new Promise<void>(resolve => h.resume = resolve); return <button id='start' onClick={async () => { await gate; debounce(() => h.values.calls = 1, 120)(); h.ready = true; }}>start</button>; }
function App() { const [shown, setShown] = createSignal(true); h.hide = () => { setShown(false); flush(); }; return <Show when={shown()}><Child /></Show>; } h.dispose = render(() => <App />, document.getElementById('root')!);`,
  flow: async page => { await page.locator('#start').click(); await page.evaluate(async () => { const h = globalThis.__experiment; h.hide(); await Promise.resolve(); h.resume(); }); await page.waitForFunction(() => globalThis.__experiment.ready); await page.waitForTimeout(160); await end(page); await snapshot(page); },
  provenance: { role: 'target', expectedIssue: true, nativeMiss: false, expectedKind: 'timeout', childDisposal: true } });
const box = `const box = document.createElement('div'); box.style.cssText = 'width:100px;height:20px'; document.body.appendChild(box); onCleanup(() => box.remove());`;
for (const [id, pkg, imports, setup, operation, kind] of [
  ['interval', 'timer', `import * as Timer from '@solid-primitives/timer';`, '', `const stop = Timer.makeTimer(() => h.values.calls = (h.values.calls ?? 0) + 1, 25, setInterval);`, 'interval'],
  ['listener', 'event-listener', `import { makeEventListener as listen } from '@solid-primitives/event-listener';`, `const target = new EventTarget(); h.dispatch = () => target.dispatchEvent(new Event('test'));`, `const stop = listen(target, 'test', () => h.values.calls = (h.values.calls ?? 0) + 1);`, 'event-listener'],
  ['resize', 'resize-observer', `import { makeResizeObserver } from '@solid-primitives/resize-observer';`, box, `const observer = makeResizeObserver(() => h.values.calls = (h.values.calls ?? 0) + 1); observer.observe(box); const stop = () => observer.unobserve(box);`, 'ResizeObserver'],
  ['intersection', 'intersection-observer', `import { makeIntersectionObserver } from '@solid-primitives/intersection-observer';`, box, `const observer = makeIntersectionObserver([box], () => h.values.calls = (h.values.calls ?? 0) + 1); const stop = () => observer.stop();`, 'IntersectionObserver'],
  ['mutation', 'mutation-observer', `import { createMutationObserver } from '@solid-primitives/mutation-observer';`, box, `const [, observer] = createMutationObserver(box, { attributes: true }, () => h.values.calls = (h.values.calls ?? 0) + 1); observer.start(); const stop = () => observer.stop();`, 'MutationObserver'],
]) for (const role of ['target', 'control']) cases.push({ id: `automatic-life-${id}-${role}`, package: '@solid-primitives/' + pkg,
  source: source(imports, `${setup} let clear: (() => void) | undefined; onCleanup(() => clear?.());`,
    `async () => { await Promise.resolve(); ${operation} ${role === 'control' ? 'clear = stop;' : ''} h.release = stop; h.ready = true; }`),
  flow: async page => { await page.locator('#start').click(); await page.waitForFunction(() => globalThis.__experiment.ready);
    // Initial native observer delivery is scheduled by the browser. Establish
    // it before disposal so timing does not masquerade as a behavior change.
    if (['resize', 'intersection'].includes(id)) await page.waitForFunction(() => globalThis.__experiment.values.calls >= 1);
    await end(page);
    if (id === 'listener') await page.evaluate(() => globalThis.__experiment.dispatch());
    await page.waitForTimeout(80); await snapshot(page); await page.evaluate(() => globalThis.__experiment.release()); },
  provenance: { role, expectedIssue: role === 'target', expectedKind: kind, nativeMiss: false, sharedResource: true } });
for (const role of ['target', 'control']) cases.push({ id: `automatic-life-cross-file-${role}`, package: '@solid-primitives/scheduled',
  source: source(`${scheduled} import { later } from './later';`, `let clear: (() => void) | undefined; onCleanup(() => clear?.());`,
    `async () => { const fn = await later(() => h.values.calls = 1); ${role === 'control' ? 'clear = fn.clear;' : ''} h.ready = true; }`),
  patchCopy(root) { const code = `import { debounce } from '@solid-primitives/scheduled'; export async function later(callback: () => void) { await Promise.resolve(); const fn = debounce(callback, 120); fn(); return fn; }`;
    writeFileSync(join(root, 'src/later.ts'), code); return { generatedLocalHelper: true, source: code }; },
  flow: async page => { await page.locator('#start').click(); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); },
  provenance: { role, expectedIssue: role === 'target', expectedKind: 'timeout', nativeMiss: false, crossFile: true } });
// The opt-in native Promise delivery bridge is needed for this continuation.
cases.push({ id: 'automatic-life-promise-callback-target', package: '@solid-primitives/scheduled',
  source: source(scheduled, '', `async () => { await Promise.resolve(); await Promise.resolve().then(() => { debounce(() => h.values.calls = 1, 120)(); }); h.ready = true; }`),
  flow: async page => { await page.locator('#start').click(); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); },
  provenance: { role: 'target', expectedIssue: true, expectedKind: 'timeout', nativeMiss: false, deferredPromiseCallback: true } });
cases.push({ id: 'automatic-life-await-in-member-target', package: '@solid-primitives/timer',
  source: source(`import * as Timer from '@solid-primitives/timer';`, '', `async () => { Timer.makeTimer(() => h.values.calls = 1, await Promise.resolve(120), setTimeout); h.ready = true; }`),
  flow: async page => { await page.locator('#start').click(); await page.waitForFunction(() => globalThis.__experiment.ready); await end(page); await page.waitForTimeout(160); await snapshot(page); },
  provenance: { role: 'target', expectedIssue: true, expectedKind: 'timeout', nativeMiss: false, awaitedMemberCall: true } });
export default cases;
