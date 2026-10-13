// The helper returns the original value and never restores an owner/observer.
export function observedRead(value,site,getObserver,getOwner) {
  const metadata=typeof site==='string'?JSON.parse(site):site;
  globalThis.__asyncReadTrace?.record(metadata,{observer:!!getObserver(),owner:!!getOwner()});
  return value;
}
export function collectAsyncReadTrace() {
  const events=[],seen=new Map();
  return {events,record(site,context){
    const key=JSON.stringify([site.path,site.sourceSha256,site.start,context]);
    const prior=seen.get(key);if(prior){prior.occurrences++;return;}
    const previous=Error.stackTraceLimit;let stack;
    try{Error.stackTraceLimit=100;stack=new Error().stack??'';}finally{Error.stackTraceLimit=previous;}
    const frames=stack.split('\n').flatMap(line=>{const match=line.match(/(?:at .*?\()?((?:file:\/\/|https?:\/\/|\/).*?):(\d+):(\d+)\)?$/);
      return match?[{path:match[1],line:Number(match[2]),column:Number(match[3])}]:[];});
    const event={site,context,occurrences:1,frames,authority:false,certification:false};seen.set(key,event);events.push(event);
  }};
}
