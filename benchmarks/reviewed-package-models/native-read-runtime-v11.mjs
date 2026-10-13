// Bound owned strong history; weak identity records live with application objects.
// Eviction is coverage loss, never proof that an unobserved use is correct.
import {boundedEvidenceRecords, evidenceLimit, encodedEvidenceBytes} from './bounded-evidence-records-v1.mjs';
// Explicit async body return records provenance without inspecting or observing adoption.
// Object/function returns leave Promise settlement and result flow unproved.
// Lexical continuations and exact registered callbacks carry provenance, never Solid context.
function frames(stack){return stack.split('\n').flatMap(line=>{const match=line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);return match?[{path:match[1],line:Number(match[2]),column:Number(match[3])}]:[];});}
function capture(){const previous=Error.stackTraceLimit;try{Error.stackTraceLimit=100;return frames(new Error().stack??'');}finally{Error.stackTraceLimit=previous;}}
export function createNativeReads(options={}){
  const limits={events:evidenceLimit(options.events,256,'events'),eventBytes:evidenceLimit(options.eventBytes,4*1024*1024,'eventBytes'),metadata:evidenceLimit(options.metadata,256,'metadata'),metadataBytes:evidenceLimit(options.metadataBytes,1024*1024,'metadataBytes'),gaps:evidenceLimit(options.gaps,128,'gaps'),gapBytes:evidenceLimit(options.gapBytes,1024*1024,'gapBytes'),guards:evidenceLimit(options.guards,128,'guards'),guardBytes:evidenceLimit(options.guardBytes,1024*1024,'guardBytes')};
  const events=[],continuationGaps=[],nodes=new WeakMap(),stores=new WeakMap(),functions=new WeakSet();
  const remove=(list,value)=>{const index=list.indexOf(value);if(index>=0)list.splice(index,1);};
  const seen=boundedEvidenceRecords({maxRecords:limits.events,maxBytes:limits.eventBytes,onRemove:(_key,value)=>remove(events,value)});
  const metadata=boundedEvidenceRecords({maxRecords:limits.metadata,maxBytes:limits.metadataBytes});
  const gaps=boundedEvidenceRecords({maxRecords:limits.gaps,maxBytes:limits.gapBytes,onRemove:(_key,value)=>remove(continuationGaps,value)});
  const guards=boundedEvidenceRecords({maxRecords:limits.guards,maxBytes:limits.guardBytes});let gapSequence=0;
  const recordGap=value=>{if(gaps.set(++gapSequence,value))continuationGaps.push(value);};
  let guardEntries=0;let active=null,intentDepth=0,nextId=0,taggedCount=0,reads=0,storeReads=0,storeCount=0,nodeCount=0;let candidateCalls=0,candidateNormalReturns=0;let continuationCount=0,continuationCompleted=0,continuationRefused=0;
  function parse(value){if(typeof value!=='string')return value;const cached=metadata.get(value);if(cached!==undefined)return cached;const parsed=JSON.parse(value);metadata.set(value,parsed);return parsed;}
  function clearPending(scope){scope.pending.length=0;scope.pendingIndex?.clear();}
  function append(scope,item){
    if(scope.closed||scope.overflow)return;
    const key=JSON.stringify([item.known.id,item.storeKey??null,item.premise,item.context,item.frames,item.chain?.map(link=>[link.helper.function,link.operation,link.returnedKind??null,link.entryFrames])??null,item.callbackRegistration?.registrationId??null]);
    scope.pendingIndex??=new Map();const previous=scope.pendingIndex.get(key);
    if(previous){previous.occurrences=(previous.occurrences??1)+(item.occurrences??1);return;}
    if(scope.pending.length>=64){scope.overflow=true;clearPending(scope);return;}
    scope.pending.push(item);scope.pendingIndex.set(key,item);
  }
  function commit(site,item){
    const key=JSON.stringify([site.path,site.sourceSha256,site.start,site.projectRevision??null,item.known.id,item.storeKey??null,item.chain?.map(link=>[link.helper.function.path,link.helper.function.sha256,link.helper.function.start,link.operation.start,link.returnedKind])??null,item.callbackRegistration?[item.callbackRegistration.registrationId,item.callbackRegistration.definition.function,item.callbackRegistration.invocation.operation]:null]),previous=seen.get(key);
    if(previous){previous.occurrences+=item.occurrences??1;return;}
    const event={site,identity:{kind:'native-node',...item.known},storeKey:item.storeKey??null,nativeRead:{premise:item.premise,frames:item.frames},context:item.context,occurrences:item.occurrences??1,frames:item.frames,authority:false,certification:false};
    if(item.chain){const primitive=item.chain.every(link=>['null','undefined','boolean','number','string','bigint','symbol'].includes(link.returnedKind));event.asyncContinuation={chain:item.chain,completion:primitive?'explicit-primitive-normal-return':'explicit-normal-async-body-return',...(!primitive?{promiseSettlement:'unobserved',resultFlow:'unproved'}:{})};}
    if(item.callbackRegistration)event.callbackRegistration=item.callbackRegistration;
    // Reserve space for growth of the occurrence counter without re-encoding on every repeat.
    if(seen.set(key,event,encodedEvidenceBytes(key)+encodedEvidenceBytes(event)+64))events.push(event);else recordGap({reason:'event retention byte budget refused this observation',site});
  }
  function successful(scope){if(scope.overflow){if(scope.continuation){scope.continuation.overflow=true;clearPending(scope.continuation);}else recordGap({reason:'operation distinct observation budget exhausted',site:scope.site});return;}for(const ticket of scope.pending){const item={known:ticket.known,premise:ticket.premise,frames:ticket.frames,context:ticket.context,storeKey:ticket.storeKey,occurrences:ticket.occurrences??1,...(ticket.chain?{chain:ticket.chain}:{}),...(ticket.callbackRegistration?{callbackRegistration:ticket.callbackRegistration}:{})};
    if(scope.continuation)append(scope.continuation,{...item,chain:[{helper:scope.continuation.helper,invocationId:scope.continuation.id,operation:scope.operation,entryFrames:scope.continuation.entryFrames}]});else commit(scope.site,item);
  }}
  return {events,continuationGaps,get taggedCount(){return taggedCount;},get stats(){return {nodes:nodeCount,storeTargets:storeCount,functions:taggedCount,nativeReadEntries:reads,nativeStoreEntries:storeReads,packageGuardEntries:guardEntries,continuationCount,continuationCompleted,continuationRefused,candidateCalls,candidateNormalReturns,eventRecords:events.length,seenRecords:seen.size,metadataRecords:metadata.size,gapRecords:continuationGaps.length,guardRecords:guards.size,observationsComplete:false,retention:{events:seen.stats,metadata:metadata.stats,gaps:gaps.stats,guards:guards.stats}};},
    tag(fn,node,premise){
      if(typeof fn!=='function'||!node||typeof node!=='object')return fn;
      if(!functions.has(fn)){functions.add(fn);taggedCount++;}
      if(!nodes.has(node)){nodeCount++;nodes.set(node,{id:++nextId,premise:parse(premise),creationFrames:capture()});}
      return fn;
    },tagStore(target,premise){
      if(target&&typeof target==='object'&&!stores.has(target)){storeCount++;stores.set(target,{id:++nextId,premise:parse(premise),creationFrames:capture(),kind:'native-store-target'});}
      return target;
    },beginStore(target,key,premise,getObserver,getOwner){
      storeReads++;if(!active||intentDepth||typeof key!=='string'||key==='then')return null;const known=stores.get(target);if(!known)return null;
      const context={observer:!!getObserver(),owner:!!getOwner()};if(context.observer||context.owner)return null;
      return {scope:active,known,premise:parse(premise),context,storeKey:key,frames:capture()};
    },beginGuard(premise,getOwner){
      guardEntries++;if(!active||intentDepth)return null;const context={observer:false,owner:!!getOwner()};if(context.owner)return null;
      premise=parse(premise);const key=JSON.stringify(premise);let known=guards.get(key);if(!known){known={id:++nextId,kind:'package-observer-guard',premise,creationFrames:[]};if(!guards.set(key,known)){recordGap({reason:'guard retention byte budget refused this identity',site:active.site});return null;}}
      return {scope:active,known,premise,context,frames:capture()};
    },enterIntent(){const previous=intentDepth;intentDepth++;return previous;},leaveIntent(previous){intentDepth=previous;},
    begin(node,premise,getObserver,getOwner){
      reads++;if(!active||intentDepth)return null;const known=nodes.get(node);if(!known)return null;
      const context={observer:!!getObserver(),owner:!!getOwner()};if(context.observer||context.owner)return null;
      return {scope:active,known,premise:parse(premise),context,frames:capture()};
    },finish(value,ticket){if(ticket)append(ticket.scope,ticket);return value;},
    currentCandidate(){return active?.candidate?active:null;},intentActive(){return !!intentDepth;},captureFrames:capture,
    enterRegisteredCallback(origin,registration){
      const token={site:origin.site,origin,previous:active,callbackRegistration:registration,pending:[],returned:false,failed:false,closed:false,overflow:false};active=token;return token;
    },finishRegisteredCallback(token){
      if(!token||token.closed)return;active=token.previous;token.closed=true;
      if(token.failed||!token.returned||token.overflow||token.origin.closed&&!token.origin.normal){recordGap({reason:token.failed?'registered callback threw':token.overflow?'registered callback observation budget exhausted':'registered callback or registration completion stays open',site:token.site});clearPending(token);return;}
      const registration={...token.callbackRegistration,returnedKind:token.returnedKind,completion:'explicit-normal-synchronous-callback-return'};
      for(const ticket of token.pending){const completed={...ticket,callbackRegistration:registration};if(token.origin.closed)successful({site:token.site,pending:[completed]});else append(token.origin,completed);}clearPending(token);
    },
    candidate(site,thunk){
      candidateCalls++;
      const parent=active,scope={candidate:true,site:parse(site),pending:[],normal:false,closed:false};active=scope;
      try{
        const value=thunk();
        scope.normal=true;candidateNormalReturns++;successful(scope);
        return value;
      }finally{scope.closed=true;clearPending(scope);active=parent;}
    },captureContinuation(helper){
      helper=parse(helper);if(!active||intentDepth)return null;
      if(active.callbackRegistration){recordGap({reason:'async continuation from a registered synchronous callback stays open',site:active.site,helper});return null;}
      if(JSON.stringify(helper.projectRevision)!==JSON.stringify(active.site.projectRevision)){continuationRefused++;recordGap({reason:'helper and consumer have different input revisions',site:active.site,helper});return null;}
      continuationCount++;return {id:continuationCount,site:active.site,helper,entryFrames:capture(),parent:active.continuation??null,parentOperation:active.operation??null,pending:[],returned:false,primitive:false,failed:false,closed:false,overflow:false};
    },captureRegisteredContinuation(origin,registration,helper){
      helper=parse(helper);if(intentDepth||origin.closed&&!origin.normal)return null;
      if(JSON.stringify(helper.projectRevision)!==JSON.stringify(origin.site.projectRevision)){continuationRefused++;recordGap({reason:'registered async callback revision differs from its origin',site:origin.site,helper});return null;}
      continuationCount++;return {id:continuationCount,site:origin.site,helper,entryFrames:capture(),parent:null,parentOperation:null,origin,callbackRegistration:registration,pending:[],returned:false,primitive:false,failed:false,closed:false,overflow:false};
    },continuation(token,operation,thunk){
      if(!token||token.closed)return thunk();const parent=active,scope={site:token.site,continuation:token,operation:parse(operation),pending:[]};active=scope;
      try{const value=thunk();successful(scope);return value;}finally{active=parent;}
    },continuationReturn(token,value){if(token){token.returned=true;token.primitive=value===null||['undefined','boolean','number','string','bigint','symbol'].includes(typeof value);token.returnedKind=value===null?'null':typeof value;}return value;},
    continuationFail(token){if(token)token.failed=true;},
    continuationFinish(token){
      if(!token||token.closed)return;token.closed=true;
      if(token.failed||!token.returned||token.overflow||token.origin?.closed&&!token.origin.normal){continuationRefused++;recordGap({reason:token.failed?'async helper threw':token.overflow?'async helper observation budget exhausted':'async helper completion has unresolved result flow',site:token.site,helper:token.helper});clearPending(token);return;}
      continuationCompleted++;
      for(const item of token.pending){for(const link of item.chain)if(link.invocationId===token.id)link.returnedKind=token.returnedKind;
        if(token.parent){if(token.parent.closed){continuationRefused++;recordGap({reason:'outer async helper finished before this completion',site:token.site,helper:token.helper});continue;}
          append(token.parent,{...item,chain:[...item.chain,{helper:token.parent.helper,invocationId:token.parent.id,operation:token.parentOperation??item.chain.at(-1).operation,entryFrames:token.parent.entryFrames}]});
        }else if(token.callbackRegistration){
          const completed={...item,callbackRegistration:{...token.callbackRegistration,returnedKind:token.returnedKind,completion:token.primitive?'explicit-primitive-normal-async-callback-return':'explicit-normal-async-callback-body-return',...(!token.primitive?{promiseSettlement:'unobserved',resultFlow:'unproved'}:{})}};
          if(token.origin.closed)commit(token.site,completed);else append(token.origin,completed);
        }else commit(token.site,item);
      }clearPending(token);
    },
  };
}
export const nativeReads=createNativeReads();globalThis.__nativeNodeReads=nativeReads;
export function withNativeReadCandidate(site,thunk){return nativeReads.candidate(site,thunk);}

export function captureAsyncContinuation(helper){return nativeReads.captureContinuation(helper);}
export function withAsyncContinuation(token,operation,thunk){return nativeReads.continuation(token,operation,thunk);}
export function asyncContinuationReturn(token,value){return nativeReads.continuationReturn(token,value);}
export function asyncContinuationFail(token){nativeReads.continuationFail(token);}
export function asyncContinuationFinish(token){nativeReads.continuationFinish(token);}
