// Callback-slot identity is an observed premise; reactive intent remains open.
import {nativeReadFeedback as currentFeedback} from './native-read-feedback-v13.mjs';
import {callbackSlotSites} from './callback-slot-sites-v1.mjs';
import {hash} from './catalog.mjs';
export function nativeReadFeedback(session,path,code,events){
  const state=session.get(path,code),accepted=[],open=[],models=new Map();
  const within=(frame,source,span)=>{if(frame?.path!==source.fileName||frame.sourceSha256!==hash(source.text))return false;const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=span.start&&offset<span.end;};
  const plain=meta=>{const {projectRevision,...rest}=meta;return rest;};
  function sourceModel(meta,span,event){const source=state.program.getSourceFile(span.path);if(!source)throw Error('callback slot source is outside the current program');if(!session.acceptRevision(meta.projectRevision).valid||JSON.stringify(meta.projectRevision)!==JSON.stringify(event.site.projectRevision))throw Error('callback slot input revision differs from its consumer');if(!models.has(source.fileName))models.set(source.fileName,callbackSlotSites(state.program,source));return {source,model:models.get(source.fileName)};}
  for(const event of events){
    if(!event.callbackRegistration){accepted.push(event);continue;}
    try{
      const registration=event.callbackRegistration;
      if(!session.acceptRevision(event.site?.projectRevision).valid)throw Error('callback registration revision was retired or was never issued');
      if(event.asyncContinuation||registration.completion!=='explicit-normal-synchronous-callback-return'||registration.identityMatched!==true||!Number.isSafeInteger(registration.registrationId)||registration.registrationId<=0)throw Error('missing synchronous callback identity and completion evidence');
      if(!['null','undefined','boolean','number','string','bigint','symbol','object','function'].includes(registration.returnedKind))throw Error('callback return kind is missing');
      const allocation=sourceModel(registration.allocation,registration.allocation.allocation,event),invocation=sourceModel(registration.invocation,registration.invocation.operation,event),definition=sourceModel(registration.definition,registration.definition.function,event);
      const equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
      const slot=allocation.model.allocations.find(item=>equal(item,plain(registration.allocation)));
      if(!slot||!slot.fields.some(field=>equal(field,registration.field)))throw Error('callback lacks an exact source parameter data slot');
      if(!invocation.model.invocations.some(item=>equal(item,plain(registration.invocation)))||registration.field.key!==registration.invocation.key)throw Error('callback lacks an exact named slot invocation');
      if(!definition.model.callbacks.some(item=>equal(item,plain(registration.definition))))throw Error('callback lacks an exact source synchronous entry');
      if(!registration.originalRegistrationFrames?.some(frame=>within(frame,allocation.source,slot.allocation))||!registration.originalRegistrationFrames.some(frame=>within(frame,state.source,event.site)))throw Error('missing mapped consumer-to-slot registration');
      if(!registration.originalInvocationFrames?.some(frame=>within(frame,invocation.source,registration.invocation.operation)))throw Error('missing mapped original slot invocation');
      if(!registration.originalEntryFrames?.some(frame=>within(frame,definition.source,registration.definition.function))||!event.nativeRead.originalFrames?.some(frame=>within(frame,definition.source,registration.definition.function)))throw Error('missing mapped callback entry and native read');
      accepted.push({...event,originalFrames:[...event.originalFrames,...registration.originalRegistrationFrames]});
    }catch(error){open.push({reason:error.message,site:event.site,authority:false,certification:false});}
  }
  const result=currentFeedback(session,path,code,accepted);
  for(const note of result.notes){const event=accepted.find(event=>event.callbackRegistration&&event.site.start===note.witness.start&&event.site.end===note.witness.end&&event.identity.id===note.nativeIdentity.id);if(!event)continue;
    note.code='OBSERVED_MEMO_REGISTERED_CALLBACK_READ';note.channel='observed-registered-callback-read';note.callbackRegistration=event.callbackRegistration;
    note.message='This callback read a reactive source after the memo stopped tracking. If that source should drive updates, capture its value during the memo compute and pass it into the callback.';
    note.basis='observed native read by the exact function stored in a fresh source data slot, matched to its original consumer call, with an explicit normal synchronous callback return';
    note.limitation='This records synchronous callback execution and normal registration return. Promise fulfillment, returned-value flow, arbitrary storage shapes and developer intent remain open.';
  }
  return {...result,open:[...open,...result.open]};
}
