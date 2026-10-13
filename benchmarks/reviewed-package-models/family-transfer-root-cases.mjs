// Repeat the transfer with setup in a root. This avoids the separate strict
// component read performed internally by the package during construction.
import originals from './family-transfer-cases.mjs';
export default originals.map(row => ({ ...row, id: row.id.replace('transfer-', 'transfer-root-'),
  source: `import { createRoot } from 'solid-js'; import { render } from '@solidjs/web';
import * as Timer from '@solid-primitives/timer'; const h = (globalThis as any).__experiment;
h.attempt('mount', () => createRoot(dispose => {
  const counter = Timer.createIntervalCounter(100000);
  const stop = render(() => <p id='value'>{'count:' + ${row.provenance.role === 'target' ? 'counter' : 'counter()'}}</p>, document.getElementById('root')!);
  h.dispose = () => { stop(); dispose(); };
}));`, provenance: { ...row.provenance, pair: 'accessor-transfer-root' }
}));
