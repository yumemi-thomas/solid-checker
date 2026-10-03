// Admit executed async-helper completion provenance before the existing projector.
import {nativeReadFeedback as currentFeedback} from './native-read-feedback-v10.mjs';
import {asyncContinuationSites} from './async-continuation-sites-v1.mjs';
import {hash} from './catalog.mjs';
export function nativeReadFeedback(session,path,code,events){
  const state=session.get(path,code),accepted=[],open=[],helpers=new Map();
  const frameWithin=(frame,source,span)=>{if(frame?.path!==source.fileName||frame.sourceSha256!==hash(source.text))return false;const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=span.start&&offset<span.end;};
  for(const event of events){
    const revision=session.acceptRevision(event.site?.projectRevision);
    if(!revision.valid){const {valid,...refusal}=revision;open.push({...refusal,site:event.site,authority:false,certification:false});continue;}
    if(!event.asyncContinuation){accepted.push(event);continue;}
    try{
      if(event.asyncContinuation.completion!=='explicit-primitive-normal-return'||!event.asyncContinuation.chain?.length)throw Error('missing completed async helper provenance');
      for(const link of event.asyncContinuation.chain){
        const metadata=link.helper,source=state.program.getSourceFile(metadata.function.path);if(!source)throw Error('async helper source is outside the current program');
        if(!session.acceptRevision(metadata.projectRevision).valid||JSON.stringify(metadata.projectRevision)!==JSON.stringify(event.site.projectRevision))throw Error('async helper input revision differs from its consumer');
        if(!helpers.has(source.fileName))helpers.set(source.fileName,asyncContinuationSites(state.program,source));
        const {projectRevision,...plain}=metadata,current=helpers.get(source.fileName).functions.find(helper=>JSON.stringify(helper)===JSON.stringify(plain));
        if(!current||!current.operations.some(operation=>JSON.stringify(operation)===JSON.stringify(link.operation)))throw Error('async helper lacks the exact current function and operation');
        if(!['null','undefined','boolean','number','string','bigint','symbol'].includes(link.returnedKind))throw Error('async helper completion value is unresolved');
        if(!link.originalEntryFrames?.some(frame=>frameWithin(frame,source,current.function)))throw Error('missing mapped async helper entry');
      }
      const inner=event.asyncContinuation.chain[0],source=state.program.getSourceFile(inner.helper.function.path);
      if(!event.nativeRead.originalFrames?.some(frame=>frameWithin(frame,source,inner.operation)))throw Error('missing mapped async helper operation');
      const outer=event.asyncContinuation.chain.at(-1);
      if(!outer.originalEntryFrames?.some(frame=>frameWithin(frame,state.source,event.site)))throw Error('missing mapped consumer-to-async-helper entry');
      // The synchronous caller frame is captured at entry, before suspension.
      accepted.push({...event,originalFrames:[...event.originalFrames,...outer.originalEntryFrames]});
    }catch(error){open.push({reason:error.message,site:event.site,authority:false,certification:false});}
  }
  const result=currentFeedback(session,path,code,accepted);
  for(const note of result.notes){const event=accepted.find(event=>event.asyncContinuation&&event.site.start===note.witness.start&&event.site.end===note.witness.end);if(!event)continue;
    note.code='OBSERVED_MEMO_CALLBACK_ASYNC_HELPER_READ';note.channel='observed-async-helper-read';note.asyncContinuation=event.asyncContinuation;
    note.basis='observed successful native read or package shortcut during an entered source async helper, followed by an explicit primitive normal return, attributed to the original memo callback call';
    note.message='This async helper read a reactive source without registering it for the memo. If that source should drive updates, capture its value during the memo compute and pass it into the helper.';
    note.limitation='Only admitted source async helpers and explicit primitive completions are covered; arbitrary promise adoption, callback timing, returned-value flow and developer intent stay open.';
  }
  return {...result,open:[...open,...result.open]};
}
