// Retain completed read provenance; suppress only a constant fulfilled whole result.
import {nativeReadFeedback as observedFeedback} from './native-read-feedback-v16.mjs';
import {asyncConstantWholeCallbackResult} from './async-constant-result-v1.mjs';
export function nativeReadFeedback(session,path,code,events){
  const observed=observedFeedback(session,path,code,events),state=session.get(path,code),notes=[],suppressed=[...observed.suppressed];
  for(const note of observed.notes){
    const model=note.asyncContinuation&&asyncConstantWholeCallbackResult(state.program,state.source,note.witness);
    const executed=model&&note.asyncContinuation.chain.some(link=>JSON.stringify(link.helper.function)===JSON.stringify(model.function));
    if(executed)suppressed.push({reason:'exact executed source async helper has a constant primitive fulfilled value, and its call is the whole callback result',model,observation:note});
    else notes.push(note);
  }
  if(!session.acceptRevision(observed.revision).valid)return {...observed,notes:[],suppressed:[],open:[...observed.open,{reason:'input revision retired during async constant-result projection',authority:false,certification:false}]};
  return {...observed,notes,suppressed};
}
