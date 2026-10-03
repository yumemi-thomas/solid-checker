// The explicit boundary states desired lifetime, never repairs package cleanup.
const cases = [];
const prelude = `import { onCleanup, flush } from 'solid-js'; import { render } from '@solidjs/web'; const h = (globalThis as any).__experiment;
const audit = (globalThis as any).__resourceAudit;
function lifetime(label: string, expectation = 'stop') { const scope = audit?.scope(label, expectation) ?? { run: (fn: Function) => fn(), bind: (fn: Function) => fn, end: () => {} }; onCleanup(() => queueMicrotask(() => scope.end())); return scope; }`;
const component = (imports, setup) => `${prelude}\n${imports}\nfunction App() { const scope = lifetime('component'); ${setup} return <p>lifetime probe</p>; } h.dispose = render(() => <App />, document.getElementById('root')!);`;
const observe = async page => page.evaluate(() => { const h = globalThis.__experiment; h.values.resourceAudit = globalThis.__resourceAudit?.snapshot() ?? null; });
const end = async page => page.evaluate(async () => { const h = globalThis.__experiment; h.dispose(); h.disposals++; await Promise.resolve(); });
function pair(id, pkg, make, flow, expectedKind) {
  for (const role of ['target', 'control']) cases.push({ id: `lifetime-${id}-${role}`, package: `@solid-primitives/${pkg}`, source: make(role === 'target'),
    flow: async (page, step) => { await flow(page, step, role === 'target'); await observe(page); },
    provenance: { pair: id, role, authoredMutation: true, expectedIssue: role === 'target', expectedKind, boundary: 'stop resources at component disposal' } });
}
for (const name of ['debounce', 'throttle', 'scheduleIdle']) pair(`${name}-event`, 'scheduled', bad => component(`import { ${name} } from '@solid-primitives/scheduled';`,
  `const callback = () => h.values.calls = (h.values.calls ?? 0) + 1;
${bad ? '' : `const scheduled = ${name}(callback, 120);`} h.start = scope.bind(() => { ${bad ? `const scheduled = ${name}(callback, 120);` : ''} scheduled(); });`),
  async (page, step) => { await step('schedule-dispose', () => page.evaluate(async () => { const h = globalThis.__experiment; h.start(); h.dispose(); h.disposals++; await Promise.resolve(); })); await page.waitForTimeout(170); }, name === 'scheduleIdle' ? 'idle-callback' : 'timeout');
pair('manual-interval', 'timer', bad => component(`import { makeTimer } from '@solid-primitives/timer';`,
  `const clear = scope.run(() => makeTimer(() => h.values.calls = (h.values.calls ?? 0) + 1, 25, setInterval)); ${bad ? '' : 'onCleanup(clear);'} h.clear = clear;`),
  async (page, step) => { await page.evaluate(() => { const h = globalThis.__experiment; h.values.before = h.values.calls ?? 0; }); await step('dispose', () => end(page)); await page.waitForTimeout(110); await page.evaluate(() => globalThis.__experiment.clear()); }, 'interval');
pair('listener-event', 'event-listener', bad => component(`import { makeEventListener } from '@solid-primitives/event-listener';`,
  `const target = new EventTarget(); const callback = () => h.values.calls = (h.values.calls ?? 0) + 1;
${bad ? '' : `scope.run(() => makeEventListener(target, 'test', callback));`} h.start = scope.bind(() => { ${bad ? `makeEventListener(target, 'test', callback);` : ''} }); h.dispatch = () => target.dispatchEvent(new Event('test'));`),
  async (page, step) => { await step('register-dispose', () => page.evaluate(async () => { const h = globalThis.__experiment; h.start(); h.dispose(); h.disposals++; await Promise.resolve(); h.dispatch(); })); }, 'event-listener');
const box = `const box = document.createElement('div'); box.style.cssText = 'width:100px;height:20px'; document.body.appendChild(box);`;
for (const [id, pkg, imported, setup, restart, kind] of [
  ['resize-restart', 'resize-observer', 'makeResizeObserver', `const observer = makeResizeObserver(() => h.values.calls = (h.values.calls ?? 0) + 1); scope.run(() => observer.observe(box));`, 'observer.observe(box)', 'ResizeObserver'],
  ['intersection-restart', 'intersection-observer', 'makeIntersectionObserver', `const observer = scope.run(() => makeIntersectionObserver([box], () => h.values.calls = (h.values.calls ?? 0) + 1));`, 'observer.start()', 'IntersectionObserver'],
  ['mutation-restart', 'mutation-observer', 'createMutationObserver', `const [, observer] = createMutationObserver(box, { attributes: true }, () => h.values.calls = (h.values.calls ?? 0) + 1); scope.run(() => observer.start());`, 'observer.start()', 'MutationObserver'],
]) pair(id, pkg, bad => component(`import { ${imported} } from '@solid-primitives/${pkg}';`,
  `${box} ${setup} h.restart = scope.bind(() => { ${bad ? restart + ';' : ''} }); h.mutate = () => box.setAttribute('data-change', '1');`),
  async (page, step) => { await page.waitForTimeout(60); await step('dispose-restart', async () => { await end(page); await page.evaluate(() => { const h = globalThis.__experiment; h.values.before = h.values.calls ?? 0; h.restart(); h.mutate(); }); }); await page.waitForTimeout(70); }, kind);

function control(id, pkg, setup, flow, extra = {}) {
  cases.push({ id: `lifetime-${id}-control`, package: `@solid-primitives/${pkg}`, source: component(extra.imports ?? '', setup),
    flow: async (page, step) => { await flow(page, step); await observe(page); }, provenance: { role: 'control', pair: id, expectedIssue: false, ...extra.provenance } });
}
control('intentional-background', 'scheduled', `const background = lifetime('background work', 'background'); h.start = background.bind(() => debounce(() => h.values.calls = (h.values.calls ?? 0) + 1, 120)());`,
  async (page, step) => { await step('schedule-dispose', () => page.evaluate(async () => { const h = globalThis.__experiment; h.start(); h.dispose(); h.disposals++; await Promise.resolve(); })); await page.waitForTimeout(170); }, { imports: `import { debounce } from '@solid-primitives/scheduled';` });
control('once-listener', 'event-listener', `const target = new EventTarget(); scope.run(() => makeEventListener(target, 'test', () => h.values.calls = (h.values.calls ?? 0) + 1, { once: true })); h.dispatch = () => target.dispatchEvent(new Event('test'));`,
  async (page, step) => { await step('dispatch-once', () => page.evaluate(() => { const h = globalThis.__experiment; h.dispatch(); h.dispatch(); })); await end(page); }, { imports: `import { makeEventListener } from '@solid-primitives/event-listener';`, provenance: { expectedGap: true } });
control('signal-listener', 'event-listener', `const target = new EventTarget(), controller = new AbortController(); h.start = scope.bind(() => makeEventListener(target, 'test', () => h.values.calls = (h.values.calls ?? 0) + 1, { signal: controller.signal })); h.abort = () => controller.abort(); h.dispatch = () => target.dispatchEvent(new Event('test'));`,
  async (page, step) => { await step('register-abort-dispose', () => page.evaluate(async () => { const h = globalThis.__experiment; h.start(); h.abort(); h.dispose(); h.disposals++; await Promise.resolve(); h.dispatch(); })); }, { imports: `import { makeEventListener } from '@solid-primitives/event-listener';`, provenance: { expectedGap: true } });
control('manual-listener-cleanup', 'event-listener', `const target = new EventTarget(); h.start = scope.bind(() => { const clear = makeEventListener(target, 'test', () => h.values.calls = (h.values.calls ?? 0) + 1); clear(); });`,
  async (page, step) => { await step('register-clear', () => page.evaluate(() => globalThis.__experiment.start())); await end(page); }, { imports: `import { makeEventListener } from '@solid-primitives/event-listener';` });
control('completed-timeout', 'timer', `scope.run(() => makeTimer(() => h.values.calls = (h.values.calls ?? 0) + 1, 10, setTimeout));`,
  async (page, step) => { await step('complete', () => page.waitForFunction(() => globalThis.__experiment.values.calls === 1)); await end(page); }, { imports: `import { makeTimer } from '@solid-primitives/timer';` });
control('interval-cross-clear', 'timer', `scope.run(() => { const id = setInterval(() => {}, 500); clearTimeout(id); });`, async page => end(page));
control('listener-options', 'event-listener', `const target = new EventTarget(); h.values.optionReads = {}; const options = Object.create(null);
for (const key of ['capture', 'once', 'passive', 'signal']) Object.defineProperty(options, key, { get() { if (this !== options) throw new Error('changed options receiver'); h.values.optionReads[key] = (h.values.optionReads[key] ?? 0) + 1; return key === 'signal' ? undefined : false; } });
scope.run(() => makeEventListener(target, 'test', function (this: EventTarget) { h.values.receiver = this === target; }, options)); target.dispatchEvent(new Event('test'));`, async page => end(page), { imports: `import { makeEventListener } from '@solid-primitives/event-listener';` });
// Promise continuation propagation is deliberately absent. Keep a type-valid
// target that proves the monitor's miss, rather than quietly excluding it.
cases.push({ id: 'lifetime-after-await-missed-target', package: '@solid-primitives/scheduled', source: component(`import { debounce } from '@solid-primitives/scheduled';`,
  `h.start = scope.bind(async () => { await Promise.resolve(); debounce(() => h.values.calls = (h.values.calls ?? 0) + 1, 120)(); });`),
  flow: async (page, step) => { await step('schedule-after-await', () => page.evaluate(async () => { const h = globalThis.__experiment; await h.start(); h.dispose(); h.disposals++; await Promise.resolve(); })); await page.waitForTimeout(170); await observe(page); },
  provenance: { role: 'target', pair: 'after-await', expectedIssue: true, expectedMiss: true, boundary: 'stop resources at component disposal' } });
export default cases;
