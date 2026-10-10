// Lexical continuation tokens carry observation provenance, never Solid context.
function frames(stack){return stack.split('\n').flatMap(line=>{const match=line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);return match?[{path:match[1],line:Number(match[2]),column:Number(match[3])}]:[];});}
function capture(){const previous=Error.stackTraceLimit;try{Error.stackTraceLimit=100;return frames(new Error().stack??'');}finally{Error.stackTraceLimit=previous;}}
export function createNativeReads(){
  const nodes=new WeakMap(),stores=new WeakMap(),functions=new WeakSet(),metadata=new Map(),events=[],seen=new Map();const guards=new Map();let guardEntries=0;let active=null,intentDepth=0,nextId=0,taggedCount=0,reads=0,storeReads=0,storeCount=0,nodeCount=0;let continuationCount=0,continuationCompleted=0,continuationRefused=0;const continuationGaps=[];
  function parse(value){if(typeof value!=='string')return value;if(!metadata.has(value))metadata.set(value,JSON.parse(value));return metadata.get(value);}
  function append(token,item){if(token.closed||token.overflow)return;if(token.pending.length>=64){token.overflow=true;token.pending.length=0;return;}token.pending.push(item);}
  function commit(site,item){
    const key=JSON.stringify([site.path,site.sourceSha256,site.start,site.projectRevision??null,item.known.id,item.storeKey??null,item.chain?.map(link=>[link.helper.function.path,link.helper.function.sha256,link.helper.function.start,link.operation.start,link.returnedKind])??null]),previous=seen.get(key);
    if(previous){previous.occurrences++;return;}
    const event={site,identity:{kind:'native-node',...item.known},storeKey:item.storeKey??null,nativeRead:{premise:item.premise,frames:item.frames},context:item.context,occurrences:1,frames:item.frames,authority:false,certification:false};
    if(item.chain)event.asyncContinuation={chain:item.chain,completion:'explicit-primitive-normal-return'};
    seen.set(key,event);events.push(event);
  }
  function successful(scope){for(const ticket of scope.pending){const item={known:ticket.known,premise:ticket.premise,frames:ticket.frames,context:ticket.context,storeKey:ticket.storeKey};
    if(scope.continuation)append(scope.continuation,{...item,chain:[{helper:scope.continuation.helper,invocationId:scope.continuation.id,operation:scope.operation,entryFrames:scope.continuation.entryFrames}]});else commit(scope.site,item);
  }}
  return {events,continuationGaps,get taggedCount(){return taggedCount;},get stats(){return {nodes:nodeCount,storeTargets:storeCount,functions:taggedCount,nativeReadEntries:reads,nativeStoreEntries:storeReads,packageGuardEntries:guardEntries,continuationCount,continuationCompleted,continuationRefused};},
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
      premise=parse(premise);const key=JSON.stringify(premise);if(!guards.has(key))guards.set(key,{id:++nextId,kind:'package-observer-guard',premise,creationFrames:[]});
      return {scope:active,known:guards.get(key),premise,context,frames:capture()};
    },enterIntent(){const previous=intentDepth;intentDepth++;return previous;},leaveIntent(previous){intentDepth=previous;},
    begin(node,premise,getObserver,getOwner){
      reads++;if(!active||intentDepth)return null;const known=nodes.get(node);if(!known)return null;
      const context={observer:!!getObserver(),owner:!!getOwner()};if(context.observer||context.owner)return null;
      return {scope:active,known,premise:parse(premise),context,frames:capture()};
    },finish(value,ticket){if(ticket)ticket.scope.pending.push(ticket);return value;},
    candidate(site,thunk){
      const parent=active,scope={site:parse(site),pending:[]};active=scope;
      try{
        const value=thunk();
        successful(scope);
        return value;
      }finally{active=parent;}
    },captureContinuation(helper){
      helper=parse(helper);if(!active||intentDepth)return null;
      if(JSON.stringify(helper.projectRevision)!==JSON.stringify(active.site.projectRevision)){continuationRefused++;continuationGaps.push({reason:'helper and consumer have different input revisions',site:active.site,helper});return null;}
      continuationCount++;return {id:continuationCount,site:active.site,helper,entryFrames:capture(),parent:active.continuation??null,parentOperation:active.operation??null,pending:[],returned:false,primitive:false,failed:false,closed:false,overflow:false};
    },continuation(token,operation,thunk){
      if(!token||token.closed)return thunk();const parent=active,scope={site:token.site,continuation:token,operation:parse(operation),pending:[]};active=scope;
      try{const value=thunk();successful(scope);return value;}finally{active=parent;}
    },continuationReturn(token,value){if(token){token.returned=true;token.primitive=value===null||['undefined','boolean','number','string','bigint','symbol'].includes(typeof value);token.returnedKind=value===null?'null':typeof value;}return value;},
    continuationFail(token){if(token)token.failed=true;},
    continuationFinish(token){
      if(!token||token.closed)return;token.closed=true;
      if(token.failed||!token.returned||!token.primitive||token.overflow){continuationRefused++;continuationGaps.push({reason:token.failed?'async helper threw':token.overflow?'async helper observation budget exhausted':'async helper completion has unresolved result flow',site:token.site,helper:token.helper});token.pending.length=0;return;}
      continuationCompleted++;
      for(const item of token.pending){for(const link of item.chain)if(link.invocationId===token.id)link.returnedKind=token.returnedKind;
        if(token.parent){if(token.parent.closed){continuationRefused++;continuationGaps.push({reason:'outer async helper finished before this completion',site:token.site,helper:token.helper});continue;}
          append(token.parent,{...item,chain:[...item.chain,{helper:token.parent.helper,invocationId:token.parent.id,operation:token.parentOperation??item.chain.at(-1).operation,entryFrames:token.parent.entryFrames}]});
        }else commit(token.site,item);
      }token.pending.length=0;
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
