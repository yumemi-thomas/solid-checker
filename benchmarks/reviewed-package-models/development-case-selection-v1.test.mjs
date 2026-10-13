import test from 'node:test';
import assert from 'node:assert/strict';
import {selectDevelopmentCases,validateDevelopmentPopulation} from './development-case-selection-v1.mjs';
const cases=[{id:'target'},{id:'control'},{id:'other'}];
const selection={caseIds:['target','control'],availableCases:3,developmentOnly:true};
test('selection preserves the original case objects and requested order',()=>{
  const rows=selectDevelopmentCases(cases,['control','target']);assert.equal(rows[0],cases[1]);assert.equal(rows[1],cases[0]);
});
test('absent selection keeps the complete population',()=>assert.equal(selectDevelopmentCases(cases),cases));
test('empty, unknown, duplicate and malformed selections refuse',()=>{
  for(const ids of [[],['missing'],['target','target'],'target',[null]])assert.throws(()=>selectDevelopmentCases(cases,ids));
});
test('duplicate input IDs refuse',()=>assert.throws(()=>selectDevelopmentCases([{id:'target'},{id:'target'}],['target'])));
test('focused observed run can reuse an authenticated full plain population',()=>{
  const rows=validateDevelopmentPopulation(cases,cases.slice(0,2),cases,selection);assert.deepEqual(rows,cases.slice(0,2));
});
test('focused baseline is also admitted',()=>assert.equal(validateDevelopmentPopulation(cases,cases.slice(0,2),cases.slice(0,2),selection).length,2));
test('missing, extra and repeated observations or baseline rows refuse',()=>{
  for(const rows of [cases.slice(0,1),cases,[cases[0],cases[0]]])assert.throws(()=>validateDevelopmentPopulation(cases,rows,cases,selection));
  for(const rows of [cases.slice(0,1),[cases[0],cases[2]],[cases[0],cases[0],cases[2]]])assert.throws(()=>validateDevelopmentPopulation(cases,cases.slice(0,2),rows,selection));
});
test('selection cannot claim fresh evidence or another population size',()=>{
  for(const metadata of [{...selection,developmentOnly:false},{...selection,availableCases:4}])assert.throws(()=>validateDevelopmentPopulation(cases,cases.slice(0,2),cases,metadata));
});
