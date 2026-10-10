// Refusal checks using replayed evidence rebound to this test's issued revision.
// This validates the projector, not trace authenticity or new runtime observations.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {packageSourceSession} from './package-source-session-v2.mjs';
import {nativeReadFeedback} from './native-read-feedback-v14.mjs';
const [reportArg,outArg]=process.argv.slice(2),reportPath=resolve(reportArg),out=resolve(outArg);assert(!existsSync(out));
const report=JSON.parse(readFileSync(reportPath)),row=report.results.find(row=>row.stages.some(stage=>stage.events.some(event=>event.callbackRegistration)));assert(row);
const stage=row.stages[0],root=join(dirname(reportPath),row.id),path=join(root,'src/main.tsx'),code=readFileSync(path,'utf8'),session=packageSourceSession(root),state=session.get(path,code);assert.deepEqual(state.errors,[]);
const event=structuredClone(stage.events.find(event=>event.callbackRegistration));event.site.projectRevision=state.revision;
for(const key of ['allocation','invocation','definition'])event.callbackRegistration[key].projectRevision=state.revision;
const project=input=>nativeReadFeedback(session,path,code,[input]);assert.equal(project(event).notes.length,1);
const checks=[
  ['registration identity',event=>event.callbackRegistration.identityMatched=false],
  ['registration id',event=>event.callbackRegistration.registrationId=0],
  ['normal callback return',event=>event.callbackRegistration.completion='pending'],
  ['return kind',event=>event.callbackRegistration.returnedKind='missing'],
  ['exact data field',event=>event.callbackRegistration.field.key='other'],
  ['allocation hash',event=>event.callbackRegistration.allocation.allocation.sha256='changed'],
  ['allocation span',event=>event.callbackRegistration.allocation.allocation.start++],
  ['parameter declaration',event=>event.callbackRegistration.field.parameter.start++],
  ['exact invocation',event=>event.callbackRegistration.invocation.operation.start++],
  ['receiver declaration',event=>event.callbackRegistration.invocation.declaration.start++],
  ['exact callback',event=>event.callbackRegistration.definition.function.start++],
  ['allocation revision',event=>event.callbackRegistration.allocation.projectRevision={...state.revision,generation:999}],
  ['invocation revision',event=>event.callbackRegistration.invocation.projectRevision={...state.revision,generation:999}],
  ['callback revision',event=>event.callbackRegistration.definition.projectRevision={...state.revision,generation:999}],
  ['registration frames',event=>event.callbackRegistration.originalRegistrationFrames=[]],
  ['invocation frames',event=>event.callbackRegistration.originalInvocationFrames=[]],
  ['entry frames',event=>event.callbackRegistration.originalEntryFrames=[]],
  ['native read frames',event=>event.nativeRead.originalFrames=[]],
  ['mixed async evidence',event=>event.asyncContinuation={chain:[]}],
];
const refusals=checks.map(([name,mutate])=>{const changed=structuredClone(event);mutate(changed);const result=project(changed);assert.deepEqual(result.notes,[],name);assert.equal(result.acceptedEvents,0,name);assert(result.open.length,name);return {name,reasons:result.open.map(item=>item.reason)};});
session.invalidate();const retired=project(event);assert.deepEqual(retired.notes,[]);assert.equal(retired.acceptedEvents,0);
writeFileSync(out,JSON.stringify({authority:false,certification:false,scope:'projector refusal checks; replayed trace metadata intentionally rebound to a new issued revision before positive baseline',baselineNotes:1,refusals,retiredRejected:true},null,2)+'\n');console.log(JSON.stringify({baseline:1,refusals:refusals.length,retiredRejected:true}));
