import assert from 'node:assert/strict';
import { existsSync,readFileSync,writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { hash,read } from './catalog.mjs';
const [selectionArg,validationArg,outputArg]=process.argv.slice(2),selectionPath=resolve(selectionArg),validationPath=resolve(validationArg),output=resolve(outputArg);
assert(!existsSync(output)); const selection=read(selectionPath),validation=read(validationPath);
const keys=new Set(validation.results.filter(r=>r.confirmedWritePair&&r.observedOnlyWithoutObserver&&r.source?.assumptions.some(a=>a.context==='caller-unknown')&&
  !r.source?.assumptions.some(a=>a.context==='tracked-compute')).map(r=>r.package+'\0'+r.export));
const rows=selection.rows.filter(r=>keys.has(r.package+'\0'+r.export)); assert(rows.length);
writeFileSync(output,JSON.stringify({...selection,rows,parentSelectionPath:selectionPath,parentSelectionSha256:hash(readFileSync(selectionPath)),
  historyPath:validationPath,historySha256:hash(readFileSync(validationPath)),summary:{admittedExports:rows.length,admittedPackages:new Set(rows.map(r=>r.package)).size}},null,2)+'\n');
console.log(JSON.stringify({exports:rows.length}));
