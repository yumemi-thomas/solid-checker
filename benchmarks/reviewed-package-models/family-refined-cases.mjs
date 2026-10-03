// Preserve the full earlier population and its flows. One authoring edit makes
// snapshot intent visible through the real API; expected labels are not inputs
// to either the runtime collector or the source analyzer.
import primary from './family-matrix-cases.mjs';
import extension from './family-matrix-extension.mjs';
import transfer from './family-transfer-permission-cases.mjs';
export const authoringEdits = [{ id: 'family-intentional-snapshot-control',
  reason: 'express the existing one-time snapshot intent with solid-js.untrack' }];
export default [...primary, ...extension, ...transfer].map(row => row.id !== authoringEdits[0].id ? row : {
  ...row, source: `import { untrack as explicitSnapshot } from 'solid-js';\n` +
    row.source.replace('const frozen = value();', 'const frozen = explicitSnapshot(value);'),
  provenance: { ...row.provenance, explicitSnapshot: true, authoringEdit: authoringEdits[0].reason },
});
