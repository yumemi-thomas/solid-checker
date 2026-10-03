// Use only an explicit authored assertion; never derive an answer from the
// proposed repair. These benchmark assertions do not represent arbitrary apps.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
import {replayAssertionFeedback} from './replay-assertion-feedback-v1.mjs';
const [auditArg,outArg]=process.argv.slice(2),auditPath=resolve(auditArg),out=resolve(outArg);assert(!existsSync(out));const audit=read(auditPath);assert(audit.completedAt&&audit.authority===false&&audit.certification===false);for(const pin of audit.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256);assert.equal(hash(readFileSync(audit.validator.path)),audit.validator.sha256);
const prepared=read(audit.inputs[0].path),assertions=new Map(prepared.rows.map(row=>[row.id,row.desired])),result=replayAssertionFeedback(audit.rows,assertions);
const noProposal=prepared.rows.filter(row=>!row.repaired).map(row=>({id:row.id,reason:row.plans.some(item=>item.open)?'the source proposal is refused':row.originalValue!==row.desired?'the failing authored assertion has no observed source witness':'no source proposal or actionable failing assertion',sourceOpen:row.plans.filter(item=>item.open).map(item=>item.open.reason),authority:false,certification:false}));
const source=new URL('./replay-assertion-feedback-v1.mjs',import.meta.url).pathname;writeFileSync(out,JSON.stringify({completedAt:new Date().toISOString(),authority:false,certification:false,assertionProvenance:'explicit desired primary values in the authored benchmark inputs; no real-application assertions are claimed',inputs:[{path:auditPath,sha256:hash(readFileSync(auditPath))},{path:source,sha256:hash(readFileSync(source))}],...result,noProposal},null,2)+'\n');console.log(JSON.stringify({proposals:audit.rows.length,notes:result.notes.length,open:result.open.length,alreadyPassing:result.unchanged.length,noProposal:noProposal.length}));
