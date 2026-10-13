// A runtime-backed intent hint, not a package contract or proven violation.
import {hash} from './catalog.mjs';
import {asyncReadSites} from './async-read-sites-v1.mjs';
export function asyncReadFeedback(program,source,events){
  const admitted=asyncReadSites(program,source),notes=[],open=[...admitted.open],seen=new Set();
  for(const event of events){
    const site=event.site;
    if(site?.path!==source.fileName||site.sourceSha256!==hash(source.text)){
      open.push({reason:'read observation belongs to different consumer bytes'});continue;
    }
    const current=admitted.sites.find(item=>item.start===site.start&&item.end===site.end);
    if(!current||JSON.stringify(current)!==JSON.stringify(site)){
      open.push({reason:'read observation lacks the exact current accessor and memo witness'});continue;
    }
    if(event.context?.observer!==false||event.context?.owner!==false)continue;
    const key=site.start+':'+site.end;if(seen.has(key))continue;seen.add(key);
    notes.push({code:'OBSERVED_MEMO_CALLBACK_UNTRACKED_READ',severity:'info',category:'intent-open',channel:'observed-reactive-read',
      authority:false,certification:false,staticDispatch:'open',basis:'exact native accessor observed without owner or observer in a callback created inside a memo',
      message:'This reactive read did not register a dependency for the memo. If the result should follow this source, read it during the memo compute and pass the captured value into the callback.',
      location:{path:source.fileName,line:site.line,column:site.column,startByte:Buffer.byteLength(source.text.slice(0,site.start))},
      start:site.start,end:site.end,witness:site,observedContext:event.context,occurrences:event.occurrences,
      limitation:'The package callback timing and returned value flow remain open; deliberate snapshots are possible.'});
  }
  return {notes,open};
}
