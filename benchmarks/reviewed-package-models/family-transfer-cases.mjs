// Transfer the same returned-accessor projection to another installed package
// and namespace import. No per-export detector or contract is added.
import originals from './family-matrix-cases.mjs';
const installation = originals.find(row => row.id === 'family-timer-life-control');
export default [true, false].map(bad => ({ id: `family-accessor-transfer-${bad ? 'target' : 'control'}`,
  package: installation.package, app: installation.app,
  source: `import { render } from '@solidjs/web'; import * as Timer from '@solid-primitives/timer';
const h = (globalThis as any).__experiment;
function App() { const counter = Timer.createIntervalCounter(100000); return <p id='value'>{'count:' + ${bad ? 'counter' : 'counter()'}}</p>; }
h.dispose = render(() => <App />, document.getElementById('root')!);`,
  flow: async page => { await page.evaluate(() => { const h = globalThis.__experiment;
    h.values.behavior = { desired: 'count:0', actual: document.getElementById('value')?.textContent ?? null }; h.dispose(); h.disposals++; }); },
  provenance: { pair: 'accessor-transfer', family: 'accessor-value', role: bad ? 'target' : 'control', expectedIssue: bad,
    rules: ['uncalled-accessor'], behaviorExpectation: 'render the numeric package accessor value' }
}));
