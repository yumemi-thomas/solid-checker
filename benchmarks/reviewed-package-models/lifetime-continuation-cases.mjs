const cases = [];
const source = setup => `import { onCleanup } from 'solid-js'; import { render } from '@solidjs/web'; import { debounce } from '@solid-primitives/scheduled';
const h = (globalThis as any).__experiment, audit = (globalThis as any).__resourceAudit;
function App() { const scope = audit?.scope('component') ?? { run: (fn: Function) => fn(), bind: (fn: Function) => fn, end: () => {} };
onCleanup(() => queueMicrotask(() => scope.end())); ${setup} return <p>continuation probe</p>; } h.dispose = render(() => <App />, document.getElementById('root')!);`;
const snapshot = async page => page.evaluate(() => { globalThis.__experiment.values.resourceAudit = globalThis.__resourceAudit?.snapshot() ?? null; });
for (const role of ['target', 'control']) cases.push({ id: `lifetime-explicit-continuation-${role}`, package: '@solid-primitives/scheduled',
  source: source(`let clear: (() => void) | undefined; onCleanup(() => clear?.()); h.start = scope.bind(async () => { await Promise.resolve(); scope.run(() => { const fn = debounce(() => h.values.calls = (h.values.calls ?? 0) + 1, 120); ${role === 'control' ? 'clear = fn.clear;' : ''} fn(); }); });`),
  flow: async (page, step) => { await step('continue-dispose', () => page.evaluate(async () => { const h = globalThis.__experiment; await h.start(); h.dispose(); h.disposals++; await Promise.resolve(); })); await page.waitForTimeout(170); await snapshot(page); },
  provenance: { role, pair: 'explicit-continuation', expectedIssue: role === 'target', expectedKind: 'timeout', boundary: 'stop resources at component disposal', explicitContinuation: true } });
cases.push({ id: 'lifetime-late-continuation-target', package: '@solid-primitives/scheduled',
  source: source(`const gate = new Promise<void>(resolve => h.resume = resolve); h.start = scope.bind(async () => { await gate; scope.run(() => debounce(() => h.values.calls = (h.values.calls ?? 0) + 1, 120)()); });`),
  flow: async (page, step) => { await step('dispose-before-continuation', () => page.evaluate(async () => { const h = globalThis.__experiment; const pending = h.start(); h.dispose(); h.disposals++; await Promise.resolve(); h.resume(); await pending; })); await page.waitForTimeout(170); await snapshot(page); },
  provenance: { role: 'target', pair: 'late-continuation', expectedIssue: true, expectedKind: 'timeout', explicitContinuation: true } });
cases.push({ id: 'lifetime-concurrent-background-control', package: '@solid-primitives/scheduled',
  source: source(`const gate = new Promise<void>(resolve => h.resume = resolve); h.start = scope.bind(async () => { await gate; }); h.background = () => debounce(() => h.values.calls = (h.values.calls ?? 0) + 1, 120)();`),
  flow: async (page, step) => { await step('unrelated-work-during-await', () => page.evaluate(async () => { const h = globalThis.__experiment; const pending = h.start(); h.background(); h.dispose(); h.disposals++; await Promise.resolve(); h.resume(); await pending; })); await page.waitForTimeout(170); await snapshot(page); },
  provenance: { role: 'control', pair: 'concurrent-background', expectedIssue: false, backgroundOutsideScope: true } });
export default cases;
