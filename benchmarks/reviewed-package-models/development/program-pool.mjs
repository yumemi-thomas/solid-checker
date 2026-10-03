import assert from 'node:assert/strict';
import {hash} from '../catalog.mjs';
import {validateProjectInputs} from '../project-read-session-v2.mjs';
// In-memory TS programs only: exact paths, full resolution inputs and producer
// identity. No shared ASTs between different binding contexts or disk programs.
export function programPool({capacity=2,validate=validateProjectInputs}={}){
  assert(Number.isSafeInteger(capacity)&&capacity>0);
  const entries=new Map(),stats={builds:0,hits:0,evictions:0};
  return {stats,clear(){entries.clear();},get(identity,inputs,build){
    assert(identity&&Object.keys(identity).length,'Producer identity is required');
    assert(inputs.length,'Recorded inputs are required');
    const key=hash(JSON.stringify({identity,inputs}));
    if(!validate(inputs).valid){entries.delete(key);throw Error('Recorded program inputs changed');}
    if(entries.has(key)){
      const value=entries.get(key);entries.delete(key);entries.set(key,value);stats.hits++;
      return {value,reused:true,key};
    }
    const value=build();assert(!value||typeof value.then!=='function');
    assert(validate(inputs).valid,'Inputs changed during program construction');
    entries.set(key,value);stats.builds++;
    if(entries.size>capacity){entries.delete(entries.keys().next().value);stats.evictions++;}
    return {value,reused:false,key};
  }};
}
