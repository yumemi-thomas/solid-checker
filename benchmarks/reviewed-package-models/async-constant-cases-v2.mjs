// Retain every V1 challenge and score. The JavaScript source-write case uses
// noEmit so TypeScript checks it without treating the input as an output file.
// Neither the proof nor any challenge source changed after its first execution.
import previous from './async-constant-cases-v1.mjs';
export default previous.map(row=>row.id==='async-constant-written-javascript-target'?{...row,noEmit:true}:row);
