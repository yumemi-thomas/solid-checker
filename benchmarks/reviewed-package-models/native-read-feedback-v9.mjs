// Retain observed reads; suppress only a bounded constant whole-result claim.
import {nativeReadFeedback as observedFeedback} from './native-read-feedback-v8.mjs';
import {constantWholeCallbackResult} from './constant-call-result-v1.mjs';
export function nativeReadFeedback(program,source,events){
  const observed=observedFeedback(program,source,events),notes=[],suppressed=[];
  for(const note of observed.notes){
    const model=constantWholeCallbackResult(program,source,note.witness);
    if(model)suppressed.push({reason:'exact source call always returns the same primitive, and its value is the whole callback result',model,observation:note});
    else notes.push(note);
  }
  return {...observed,notes,suppressed};
}
