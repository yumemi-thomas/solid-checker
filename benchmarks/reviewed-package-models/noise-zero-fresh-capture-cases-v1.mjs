// Fresh generated proposals measure results without consuming expected values.
import {read} from './catalog.mjs';
const prepared=read('rust/target/noise-zero-fresh-prepared-v1.json');
export const provenance=prepared.inputs;
export default prepared.rows.filter(row=>row.repaired).map(row=>row.repaired);
