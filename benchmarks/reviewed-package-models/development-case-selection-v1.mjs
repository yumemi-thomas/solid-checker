// Selection narrows development execution; it grants no evidence or fresh-case status.
import assert from 'node:assert/strict';

export function selectDevelopmentCases(cases,ids){
  const available=new Map(cases.map(row=>[row.id,row]));
  assert.equal(available.size,cases.length,'duplicate input case');
  if(ids===undefined)return cases;
  assert(Array.isArray(ids)&&ids.length>0,'selection must contain case IDs');
  assert(ids.every(id=>typeof id==='string'&&available.has(id)),'unknown case ID');
  assert.equal(new Set(ids).size,ids.length,'duplicate selected case');
  return ids.map(id=>available.get(id));
}

export function validateDevelopmentPopulation(cases,observed,plain,selection){
  const selected=selectDevelopmentCases(cases,selection?.caseIds);
  if(selection){assert.equal(selection.developmentOnly,true);assert.equal(selection.availableCases,cases.length);}
  const sorted=rows=>rows.map(row=>row.id).sort();
  assert.deepEqual(sorted(observed),sorted(selected),'observed population differs from selection');
  const plainIds=sorted(plain);
  if(plain.length===cases.length)assert.deepEqual(plainIds,sorted(cases),'full baseline population differs');
  else assert.deepEqual(plainIds,sorted(selected),'focused baseline population differs');
  return selected;
}
