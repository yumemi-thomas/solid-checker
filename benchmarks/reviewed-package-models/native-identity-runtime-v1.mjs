// Observational identities; native function objects and reactive contexts survive.
function frames(stack){return stack.split('\n').flatMap(line=>{const match=line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);return match?[{path:match[1],line:Number(match[2]),column:Number(match[3])}]:[];});}
function capture(){const previous=Error.stackTraceLimit;try{Error.stackTraceLimit=100;return frames(new Error().stack??'');}finally{Error.stackTraceLimit=previous;}}
export function createNativeIdentities(){
  const accessors=new WeakMap(),metadata=new Map(),events=[],seen=new Map();let nextId=0;
  return {events,get taggedCount(){return nextId;},tag(fn,premise,getObserver,getOwner){
    if(typeof fn!=='function'||typeof getObserver!=='function'||typeof getOwner!=='function')return fn;
    if(!accessors.has(fn)){
      if(typeof premise==='string'){if(!metadata.has(premise))metadata.set(premise,JSON.parse(premise));premise=metadata.get(premise);}
      accessors.set(fn,{id:++nextId,premise,frames:capture(),getObserver,getOwner});
    }
    return fn;
  },call(fn,site){
    const known=accessors.get(fn),context=known?{observer:!!known.getObserver(),owner:!!known.getOwner()}:null,value=fn();
    if(!known)return value;
    if(context.observer||context.owner)return value;
    if(typeof site==='string'){if(!metadata.has(site))metadata.set(site,JSON.parse(site));site=metadata.get(site);}
    const key=JSON.stringify([site.path,site.sourceSha256,site.start,known.id]),previous=seen.get(key);
    if(previous)previous.occurrences++;
    else{const event={site,identity:{id:known.id,premise:known.premise,creationFrames:known.frames},context,occurrences:1,frames:capture(),authority:false,certification:false};seen.set(key,event);events.push(event);}
    return value;
  }};
}
// Import this module before application modules create their accessors.
export const nativeIdentities=createNativeIdentities();
globalThis.__nativeAccessorIdentities=nativeIdentities;
export function callNativeCandidate(fn,site){return nativeIdentities.call(fn,site);}
