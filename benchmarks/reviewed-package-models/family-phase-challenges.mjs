// Additional consumers stress the generic phase join independently of the
// original forty-five. One named-callback boundary is intentionally retained.
import originals from './family-matrix-extension.mjs';
const installation = originals.find(row => row.id === 'family-after-await-target');
const scenarios = [
  { id: 'namespace-await', role: 'target', imported: `import * as Memo from '@solid-primitives/memo';`, callee: 'Memo.createLazyMemo',
    callback: `async () => { await Promise.resolve(); return count(); }`, desired: '2' },
  { id: 'alias-await', role: 'target', imported: `import { createLazyMemo as lazy } from '@solid-primitives/memo';`, callee: 'lazy',
    callback: `async () => { await Promise.resolve(); return count(); }`, desired: '2' },
  { id: 'conditional-await', role: 'control', callback: `async () => { if (false) await Promise.resolve(); return count(); }`, desired: '2' },
  { id: 'nested-await', role: 'control', callback: `() => { const later = async () => { await Promise.resolve(); return 7; }; void later(); return count(); }`, desired: '2' },
  { id: 'explicit-untracked-await', role: 'control', callback: `async () => { await Promise.resolve(); return untrack(count); }`, desired: '1' },
  { id: 'captured-snapshot-await', role: 'control', callback: `async () => { const captured = untrack(count); await Promise.resolve(); return captured; }`, desired: '1' },
  { id: 'named-callback-open', role: 'target', setup: `const compute = async () => { await Promise.resolve(); return count(); };`,
    callback: 'compute', desired: '2', expectedGap: true },
];
export default scenarios.map(scenario => ({ id: 'phase-challenge-' + scenario.id, package: installation.package, app: installation.app,
  source: `import { createSignal, flush, Loading, untrack } from 'solid-js'; import { render } from '@solidjs/web';
${scenario.imported ?? "import { createLazyMemo } from '@solid-primitives/memo';"} const h = (globalThis as any).__experiment;
function App() { const [count, setCount] = createSignal(1); h.update = () => { setCount(2); flush(); };
${scenario.setup ?? ''} const value = ${scenario.callee ?? 'createLazyMemo'}(${scenario.callback});
return <Loading fallback={<p>waiting</p>}><p id='value'>{String(value())}</p></Loading>; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
  flow: async page => { await page.waitForFunction(() => document.getElementById('value')?.textContent === '1', null, { timeout: 5000 });
    await page.evaluate(() => globalThis.__experiment.update()); await page.waitForTimeout(50);
    await page.evaluate(desired => { const h = globalThis.__experiment;
      h.values.behavior = { desired, actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; }, scenario.desired); },
  provenance: { role: scenario.role, expectedIssue: scenario.role === 'target', family: 'phase-challenge',
    rules: scenario.role === 'target' ? ['reactive-read-after-await'] : [], expectedGap: scenario.expectedGap ?? false }
}));
