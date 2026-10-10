// Additional controls keep the first executed corpus and detector bytes intact.
import original from './family-matrix-cases.mjs';
const snapshot = original.find(row => row.id === 'family-intentional-snapshot-control');
const cases = [{ ...snapshot, id: 'family-explicit-snapshot-control',
  source: `import { untrack } from 'solid-js';\n` + snapshot.source.replace('const frozen = value();', 'const frozen = untrack(value);'),
  provenance: { ...snapshot.provenance, explicitSnapshot: true } }];
for (const bad of [true, false]) cases.push({
  id: `family-after-await-${bad ? 'target' : 'control'}`, package: snapshot.package, app: snapshot.app,
  source: `import { createSignal, flush, Loading } from 'solid-js'; import { render } from '@solidjs/web';
import { createLazyMemo } from '@solid-primitives/memo'; const h = (globalThis as any).__experiment;
function App() { const [count, setCount] = createSignal(1); h.update = () => { setCount(2); flush(); };
const value = createLazyMemo(async () => { ${bad ? 'await Promise.resolve(); return count();' : 'const captured = count(); await Promise.resolve(); return captured;'} });
return <Loading fallback={<p>waiting</p>}><p id='value'>{String(value())}</p></Loading>; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
  flow: async page => { await page.waitForFunction(() => document.getElementById('value')?.textContent === '1', null, { timeout: 5000 });
    await page.evaluate(() => globalThis.__experiment.update()); await page.waitForTimeout(50);
    await page.evaluate(() => { const h = globalThis.__experiment;
      h.values.behavior = { desired: '2', actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; }); },
  provenance: { pair: 'after-await', family: 'escaped-async', role: bad ? 'target' : 'control', expectedIssue: bad,
    rules: ['reactive-read-after-await'], behaviorExpectation: 'async result follows subsequent source changes' }
});
export default cases;
