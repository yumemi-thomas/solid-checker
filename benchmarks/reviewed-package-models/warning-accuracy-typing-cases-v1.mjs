// Adapted typing-boundary controls from the earlier published queue population.
import previous from './async-callback-slot-cases-v1.mjs';
export default previous.filter(row=>row.stages[0].role==='typing-exclusion');
