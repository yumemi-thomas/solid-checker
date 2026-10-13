// A session-issued input revision precedes all source-based hint projection.
import {nativeReadFeedback as projectFeedback} from './native-read-feedback-v9.mjs';
export function nativeReadFeedback(session,path,code,events){
  const state=session.get(path,code),accepted=[],open=[];
  for(const event of events){
    const revision=event.site?.projectRevision,validation=session.acceptRevision(revision);
    if(!validation.valid){const {valid,...refusal}=validation;open.push({...refusal,site:event.site,authority:false,certification:false});continue;}
    const {projectRevision,...site}=event.site;
    accepted.push({...event,site});
  }
  const result=projectFeedback(state.program,state.source,accepted);
  // Validate again: source-backed premises must stay current through projection.
  const validation=session.acceptRevision(state.revision);
  if(!validation.valid)return {notes:[],suppressed:[],open:[...open,{...validation,reason:'project changed while projecting feedback'}],
    revision:state.revision,acceptedEvents:0,authority:false,certification:false};
  return {...result,open:[...open,...result.open],revision:state.revision,acceptedEvents:accepted.length,authority:false,certification:false};
}
