// Native GC is an observed control, never an assumption that a target vanished.
const prelude = `import { makeEventListener } from '@solid-primitives/event-listener';
const h = (globalThis as any).__experiment;
const scope = (globalThis as any).__resourceAudit?.scope('GC witness') ?? { run: (fn: () => void) => fn(), end() {} };
h.end = () => scope.end();`;
const cases = [];
for (const role of ['collected', 'held', 'cancelled']) cases.push({ id: `gc-listener-${role}`, package: '@solid-primitives/event-listener',
  source: `${prelude}
scope.run(() => { const target = new EventTarget(); h.targetRef = new WeakRef(target);
const clear = makeEventListener(target, 'test', () => { h.values.calls = (h.values.calls ?? 0) + 1; });
${role === 'held' ? 'h.held = target; h.release = clear;' : role === 'cancelled' ? 'clear();' : ''} });`,
  flow: async (page, step) => {
    const session = await page.context().newCDPSession(page);
    await step('native-GC', async () => { await session.send('HeapProfiler.collectGarbage'); await page.waitForTimeout(10); await session.send('HeapProfiler.collectGarbage'); });
    await page.evaluate(() => { const h = globalThis.__experiment; h.values.collectedBeforeEnd = h.targetRef.deref() === undefined; });
    await page.evaluate(() => { const h = globalThis.__experiment; h.end(); h.values.resourceAudit = globalThis.__resourceAudit?.snapshot() ?? null;
      if (h.held) { h.held.dispatchEvent(new Event('test')); h.release(); } });
    await session.detach();
  }, provenance: { role, packageWrapper: true, explicitLifetimeExpectation: true, GCRequiredForControl: role === 'collected' } });
export default cases;
