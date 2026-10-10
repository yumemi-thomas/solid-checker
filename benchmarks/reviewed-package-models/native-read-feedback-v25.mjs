// Callback-slot identity is an observed premise; reactive intent remains open.
import {nativeReadFeedback as currentFeedback} from './native-read-feedback-v17.mjs';
import {nativeReadFeedback as sourceFeedback} from './native-read-feedback-v10.mjs';
import {asyncContinuationSites} from './async-continuation-sites-v3.mjs';
import {callbackSlotSites} from './callback-slot-sites-v2.mjs';
import {hash} from './catalog.mjs';
export function nativeReadFeedback(session,path,code,events){
  const state=session.get(path,code),accepted=[],registeredAsync=[],open=[],models=new Map(),helpers=new Map();
  const within=(frame,source,span)=>{if(frame?.path!==source.fileName||frame.sourceSha256!==hash(source.text))return false;const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=span.start&&offset<span.end;};
  const plain=meta=>{const {projectRevision,...rest}=meta;return rest;};
  function sourceModel(meta,span,event){const source=state.program.getSourceFile(span.path);if(!source)throw Error('callback slot source is outside the current program');if(!session.acceptRevision(meta.projectRevision).valid||JSON.stringify(meta.projectRevision)!==JSON.stringify(event.site.projectRevision))throw Error('callback slot input revision differs from its consumer');if(!models.has(source.fileName))models.set(source.fileName,callbackSlotSites(state.program,source));return {source,model:models.get(source.fileName)};}
  for(const event of events){
    if(!event.callbackRegistration){accepted.push(event);continue;}
    try{
      const registration=event.callbackRegistration;
      if(!session.acceptRevision(event.site?.projectRevision).valid)throw Error('callback registration revision was retired or was never issued');
      const asynchronous=registration.definition.kind==='source-async-callback-entry';
      const primitiveCallback=['null','undefined','boolean','number','string','bigint','symbol'].includes(registration.returnedKind);
      if((asynchronous?registration.completion!==(primitiveCallback?'explicit-primitive-normal-async-callback-return':'explicit-normal-async-callback-body-return')||!event.asyncContinuation||(!primitiveCallback&&(registration.promiseSettlement!=='unobserved'||registration.resultFlow!=='unproved'))||(primitiveCallback&&(registration.promiseSettlement!==undefined||registration.resultFlow!==undefined)):!!event.asyncContinuation||registration.completion!=='explicit-normal-synchronous-callback-return')||registration.identityMatched!==true||!Number.isSafeInteger(registration.registrationId)||registration.registrationId<=0)throw Error('missing callback identity or accurate body return grade');
      if(!['null','undefined','boolean','number','string','bigint','symbol','object','function'].includes(registration.returnedKind))throw Error('callback return kind is missing');
      const allocation=sourceModel(registration.allocation,registration.allocation.allocation,event),invocation=sourceModel(registration.invocation,registration.invocation.operation,event),definition=sourceModel(registration.definition,registration.definition.function,event);
      const equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
      const slot=allocation.model.allocations.find(item=>equal(item,plain(registration.allocation)));
      if(!slot||!slot.fields.some(field=>equal(field,registration.field)))throw Error('callback lacks an exact source parameter data slot');
      if(!invocation.model.invocations.some(item=>equal(item,plain(registration.invocation)))||registration.field.key!==registration.invocation.key)throw Error('callback lacks an exact named slot invocation');
      if(!definition.model.callbacks.some(item=>equal(item,plain(registration.definition))))throw Error('callback lacks an exact source entry');
      if(!registration.originalRegistrationFrames?.some(frame=>within(frame,allocation.source,slot.allocation))||!registration.originalRegistrationFrames.some(frame=>within(frame,state.source,event.site)))throw Error('missing mapped consumer-to-slot registration');
      if(!registration.originalInvocationFrames?.some(frame=>within(frame,invocation.source,registration.invocation.operation)))throw Error('missing mapped original slot invocation');
      if(!registration.originalEntryFrames?.some(frame=>within(frame,definition.source,registration.definition.function))||!asynchronous&&!event.nativeRead.originalFrames?.some(frame=>within(frame,definition.source,registration.definition.function)))throw Error('missing mapped callback entry and native read');
      if(asynchronous){
        const chain=event.asyncContinuation.chain;if(!chain?.length)throw Error('missing explicit async body return provenance');
      const primitive=chain.every(link=>['null','undefined','boolean','number','string','bigint','symbol'].includes(link.returnedKind));
      if(event.asyncContinuation.completion!==(primitive?'explicit-primitive-normal-return':'explicit-normal-async-body-return')||(!primitive&&(event.asyncContinuation.promiseSettlement!=='unobserved'||event.asyncContinuation.resultFlow!=='unproved'))||(primitive&&(event.asyncContinuation.promiseSettlement!==undefined||event.asyncContinuation.resultFlow!==undefined)))throw Error('async completion grade differs from observed return kinds or claims result settlement');
        for(const link of chain){
          const meta=link.helper,source=state.program.getSourceFile(meta.function.path);if(!source)throw Error('async callback helper source is outside the program');
          if(!session.acceptRevision(meta.projectRevision).valid||!equal(meta.projectRevision,event.site.projectRevision))throw Error('async callback helper revision differs from its registration');
          if(!helpers.has(source.fileName))helpers.set(source.fileName,asyncContinuationSites(state.program,source));
          const helper=helpers.get(source.fileName).functions.find(item=>equal(item,plain(meta)));if(!helper||!helper.operations.some(item=>equal(item,link.operation)))throw Error('missing exact registered async function and operation');
          if(!Number.isSafeInteger(link.invocationId)||link.invocationId<=0||!['null','undefined','boolean','number','string','bigint','symbol','object','function'].includes(link.returnedKind))throw Error('registered async helper body return kind stays open');
          if(!link.originalEntryFrames?.some(frame=>within(frame,source,helper.function)))throw Error('missing mapped registered async helper entry');
        }
        for(let index=0;index<chain.length;index++){const link=chain[index];if(link.helper.kind==='source-lexical-callback-helper'){const parent=chain[index+1];if(!parent||!equal(link.helper.lexicalParent,parent.helper.function)||!equal(link.helper.creationOperation,parent.operation))throw Error('lexical callback lineage differs from its exact enclosing operation');}}
        const first=chain[0],outer=chain.at(-1);if(!equal(outer.helper.function,registration.definition.function)||outer.returnedKind!==registration.returnedKind)throw Error('async helper chain does not finish at the registered callback');
        if(!event.nativeRead.originalFrames?.some(frame=>within(frame,state.program.getSourceFile(first.helper.function.path),first.operation)))throw Error('missing mapped registered async helper read operation');
        registeredAsync.push({...event,originalFrames:[...event.originalFrames,...registration.originalRegistrationFrames]});continue;
      }
      accepted.push({...event,originalFrames:[...event.originalFrames,...registration.originalRegistrationFrames]});
    }catch(error){open.push({reason:error.message,site:event.site,authority:false,certification:false});}
  }
  const existing=currentFeedback(session,path,code,accepted),asyncResult=sourceFeedback(session,path,code,registeredAsync.map(event=>{const {asyncContinuation,...rest}=event;return rest;}));
  const result={...existing,notes:[...existing.notes,...asyncResult.notes],suppressed:[...existing.suppressed,...asyncResult.suppressed],open:[...existing.open,...asyncResult.open],acceptedEvents:existing.acceptedEvents+asyncResult.acceptedEvents};
  for(const note of result.notes){const event=[...accepted,...registeredAsync].find(event=>event.callbackRegistration&&event.site.start===note.witness.start&&event.site.end===note.witness.end&&event.identity.id===note.nativeIdentity.id);if(!event)continue;
    note.code=event.asyncContinuation?'OBSERVED_MEMO_REGISTERED_ASYNC_CALLBACK_READ':'OBSERVED_MEMO_REGISTERED_CALLBACK_READ';if(event.asyncContinuation)note.asyncContinuation=event.asyncContinuation;note.channel='observed-registered-callback-read';note.callbackRegistration=event.callbackRegistration;
    note.message='This callback read a reactive source after the memo stopped tracking. If that source should drive updates, capture its value during the memo compute and pass it into the callback.';
    note.basis=event.asyncContinuation?'observed native read through a lexical helper chain with explicit normal body returns from the exact registered async callback, linked to its original consumer call; object or function returns leave Promise settlement unproved':'observed native read by the exact function stored in a fresh source data slot, matched to its original consumer call, with an explicit normal synchronous callback return';
    note.limitation='Only admitted registered callback entries, normal registration return and explicit synchronous or async body return are covered. General Promise adoption and settlement, returned-value flow, arbitrary storage shapes and developer intent remain open.';
  }
  const seen=new Set();result.notes=result.notes.filter(note=>{const key=note.witness.start+':'+note.witness.end;if(seen.has(key))return false;seen.add(key);return true;});
  if(!session.acceptRevision(state.revision).valid||JSON.stringify(existing.revision)!==JSON.stringify(state.revision)||JSON.stringify(asyncResult.revision)!==JSON.stringify(state.revision))return {...result,notes:[],suppressed:[],acceptedEvents:0,open:[...open,...result.open,{reason:'input revision changed during callback projection',authority:false,certification:false}]};
  return {...result,open:[...open,...result.open]};
}

