// Preserve the original six executed async consumers and their labels/flows.
import cases from './callback-context-transfer-cases-v1.mjs';
export default cases.filter(row=>row.provenance.family==='silent-async-dependency-loss');
