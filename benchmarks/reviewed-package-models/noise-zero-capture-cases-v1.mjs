// Generated isolated proposals. Expected values and scorer roles stay absent.
import {read} from './catalog.mjs';
const prepared=read('rust/target/noise-zero-replay-prepared-v2.json');
export const provenance=prepared.inputs;
export default prepared.rows.filter(row=>row.repaired).map(row=>row.repaired);
