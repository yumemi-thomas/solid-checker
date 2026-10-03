// Synchronous call scopes attribute actual native node reads without wrapping getters.
function frames(stack){return stack.split('\n').flatMap(line=>{const match=line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);return match?[{path:match[1],line:Number(match[2]),column:Number(match[3])}]:[];});}
function capture(){const previous=Error.stackTraceLimit;try{Error.stackTraceLimit=100;return frames(new Error().stack??'');}finally{Error.stackTraceLimit=previous;}}
export function createNativeReads(){
  const nodes=new WeakMap(),stores=new WeakMap(),functions=new WeakSet(),metadata=new Map(),events=[],seen=new Map();const guards=new Map();let guardEntries=0;let active=null,intentDepth=0,nextId=0,taggedCount=0,reads=0,storeReads=0,storeCount=0,nodeCount=0;
  function parse(value){if(typeof value!=='string')return value;if(!metadata.has(value))metadata.set(value,JSON.parse(value));return metadata.get(value);}
  return {events,get taggedCount(){return taggedCount;},get stats(){return {nodes:nodeCount,storeTargets:storeCount,functions:taggedCount,nativeReadEntries:reads,nativeStoreEntries:storeReads,packageGuardEntries:guardEntries};},
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
        for(const ticket of scope.pending){const key=JSON.stringify([scope.site.path,scope.site.sourceSha256,scope.site.start,ticket.known.id,ticket.storeKey??null]),previous=seen.get(key);
          if(previous)previous.occurrences++;
          else{const event={site:scope.site,identity:{kind:'native-node',...ticket.known},storeKey:ticket.storeKey??null,nativeRead:{premise:ticket.premise,frames:ticket.frames},
            context:ticket.context,occurrences:1,frames:ticket.frames,authority:false,certification:false};seen.set(key,event);events.push(event);}
        }
        return value;
      }finally{active=parent;}
    },
  };
}
export const nativeReads=createNativeReads();globalThis.__nativeNodeReads=nativeReads;
export function withNativeReadCandidate(site,thunk){return nativeReads.candidate(site,thunk);}

