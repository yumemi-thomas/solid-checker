const source = body => `import { onCleanup } from 'solid-js'; import { render } from '@solidjs/web'; import { makeEventListener } from '@solid-primitives/event-listener';
const h = (globalThis as any).__experiment, audit = (globalThis as any).__resourceAudit;
function App() { const scope = audit?.scope('component') ?? { run: (fn: Function) => fn(), end: () => {} }; onCleanup(() => queueMicrotask(() => scope.end())); ${body} return <p>coercion probe</p>; } h.dispose = render(() => <App />, document.getElementById('root')!);`;
const cases = [];
for (const [id, body, expectedGaps] of [
  ['coerced-cancellation', `scope.run(() => { const first = setTimeout(() => h.values.calls = 1, 30); clearTimeout(first + 0.75); const second = setTimeout(() => h.values.calls = 1, 30); clearTimeout({ valueOf() { h.values.conversions = (h.values.conversions ?? 0) + 1; return second; } } as any); });`, 2],
  ['coerced-event-type', `const target = new EventTarget(); scope.run(() => makeEventListener(target, 'test', () => h.values.calls = 1)); target.removeEventListener({ toString() { h.values.conversions = (h.values.conversions ?? 0) + 1; return 'test'; } } as any, () => {});
// The different callback removes nothing. Explicit cleanup still removes the
// real listener; the opaque type operation must stay open throughout.
const callback = () => h.values.calls = 1; scope.run(() => makeEventListener(target, 'other', callback)); target.removeEventListener({ toString() { h.values.conversions++; return 'other'; } } as any, callback);`, 2],
]) cases.push({ id: `lifetime-${id}-control`, package: '@solid-primitives/event-listener', source: source(body),
  flow: async (page, step) => { await step('dispose', () => page.evaluate(async () => { const h = globalThis.__experiment; h.dispose(); h.disposals++; await Promise.resolve(); })); await page.waitForTimeout(70); await page.evaluate(() => { const h = globalThis.__experiment; h.values.resourceAudit = globalThis.__resourceAudit?.snapshot() ?? null; }); },
  provenance: { role: 'control', pair: id, expectedIssue: false, expectedGap: true, expectedGaps } });
export default cases;
