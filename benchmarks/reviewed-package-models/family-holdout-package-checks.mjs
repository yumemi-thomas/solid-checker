// Follow-up investigation; the main heldout population remains unchanged.
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const repo = resolve(new URL('../..', import.meta.url).pathname), frozen = read(resolve(repo, 'rust/target/family-holdout-detector-freeze.json')),
  project = frozen.packages.find(row => row.package === '@solid-primitives/pagination').project;
export default ['package-same-length', 'package-different-length', 'native-same-length'].map(mode => ({
  id: 'holdout-check-' + mode, package: '@solid-primitives/pagination',
  app: relative(resolve(repo, 'rust/target/app-import-metric/apps'), project),
  source: `import { createSignal, createMemo, flush } from 'solid-js'; import { render } from '@solidjs/web';
import { createSegment } from '@solid-primitives/pagination'; const h = (globalThis as any).__experiment;
function App() { const [items, setItems] = createSignal([1, 2]);
const value = ${mode.startsWith('native') ? 'createMemo(() => items().slice(0, 2))' : 'createSegment(items, 2, () => 1)'};
h.update = () => { setItems(${mode === 'package-different-length' ? '[3, 4, 5]' : '[3, 4]'}); flush(); };
return <p id='value'>{value().join(',')}</p>; }
h.attempt('mount', () => h.dispose = render(() => <App />, document.getElementById('root')!));`,
  flow: async page => {
    await page.evaluate(() => globalThis.__experiment.update()); await page.waitForTimeout(50);
    await page.evaluate(() => { const h = globalThis.__experiment;
      h.values.behavior = { desired: '3,4', actual: document.getElementById('value')?.textContent ?? null }; h.dispose?.(); h.disposals++; });
  },
  provenance: { role: 'probe', family: 'package-internal-cache', mode, rules: [],
    question: 'Does createSegment ignore replacement contents when slice boundaries stay unchanged?' },
}));
