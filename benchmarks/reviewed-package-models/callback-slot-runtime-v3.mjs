// Keep the original property call. Activate only at a known callback's own entry.
// This records synchronous execution, not Promise fulfillment or result flow.
import {nativeReads} from './native-read-runtime-v8.mjs';
const descriptor=Object.getOwnPropertyDescriptor;
export function createCallbackSlots(reads){
  const slots=new WeakMap(),metadata=new Map();let invocation=null,nextId=0;
  const gaps=[],stats={registrations:0,entries:0,matched:0,refused:0};
  const parse=x=>{if(typeof x!=='string')return x;if(!metadata.has(x))metadata.set(x,JSON.parse(x));return metadata.get(x);};
  return {stats,gaps,
    register(value,description){description=parse(description);const origin=reads.currentCandidate?.();if(!origin||reads.intentActive?.())return value;const fields=new Map();for(const field of description.fields){const fn=descriptor(value,field.key)?.value;if(typeof fn==='function'){fields.set(field.key,{fn,field,id:++nextId,description,origin,frames:reads.captureFrames()});stats.registrations++;}}if(fields.size)slots.set(value,fields);return value;},
    invoke(receiver,key,description,thunk){const previous=invocation,record=slots.get(receiver)?.get(key);invocation=record&&descriptor(receiver,key)?.value===record.fn?{receiver,key,description:parse(description),frames:reads.captureFrames()}:null;try{return thunk();}finally{invocation=previous;}},
    match(fn,description){stats.entries++;description=parse(description);const current=invocation,record=current&&slots.get(current.receiver)?.get(current.key);if(!record||record.fn!==fn||reads.intentActive?.())return null;
      if(!record.origin.normal&&record.origin.closed||JSON.stringify(record.origin.site.projectRevision)!==JSON.stringify(description.projectRevision)||JSON.stringify(record.origin.site.projectRevision)!==JSON.stringify(current.description.projectRevision)){stats.refused++;return null;}
      stats.matched++;return {origin:record.origin,registration:{registrationId:record.id,allocation:record.description,field:record.field,definition:description,invocation:current.description,registrationFrames:record.frames,invocationFrames:current.frames,entryFrames:reads.captureFrames(),identityMatched:true}};
    },enter(fn,description){const match=this.match(fn,description);return match?reads.enterRegisteredCallback(match.origin,match.registration):null;},
    enterAsync(fn,description,helper){const match=this.match(fn,description);return match?reads.captureRegisteredContinuation(match.origin,match.registration,helper):reads.captureContinuation(helper);},returned(token,value){if(token){token.returned=true;token.returnedKind=value===null?'null':typeof value;}return value;},failed(token){if(token)token.failed=true;},finish(token){if(token)reads.finishRegisteredCallback(token);},
  };
}
export const callbackSlots=createCallbackSlots(nativeReads);globalThis.__callbackSlots=callbackSlots;
export const registerCallbackSlots=(value,meta)=>callbackSlots.register(value,meta);
export const withCallbackSlotInvocation=(receiver,key,meta,thunk)=>callbackSlots.invoke(receiver,key,meta,thunk);
export const enterRegisteredCallback=(fn,meta)=>callbackSlots.enter(fn,meta);
export const registeredCallbackReturn=(token,value)=>callbackSlots.returned(token,value);
export const registeredCallbackFail=token=>callbackSlots.failed(token);
export const registeredCallbackFinish=token=>callbackSlots.finish(token);

export const captureRegisteredAsyncCallback=(fn,meta,helper)=>callbackSlots.enterAsync(fn,meta,helper);
