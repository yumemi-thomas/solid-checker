// The published option grants the package's polling effect permission to write
// its own signal. Retain the default-option failures as separate observations.
import originals from './family-transfer-root-cases.mjs';
export default originals.map(row => ({ ...row, id: row.id.replace('transfer-root-', 'transfer-permission-'),
  source: row.source.replace('createIntervalCounter(100000)', 'createIntervalCounter(100000, { ownedWrite: true })'),
  provenance: { ...row.provenance, pair: 'accessor-transfer-permission' }
}));
