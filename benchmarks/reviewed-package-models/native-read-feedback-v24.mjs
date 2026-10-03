// Preserve raw read observations; constant returned fields do not support a
// stale-result suggestion. This says nothing about reactive side-effect intent.
import {nativeReadFeedback as previousFeedback} from './native-read-feedback-v21.mjs';
import {memoResultFieldUses} from './memo-result-field-uses-v1.mjs';
import {constantReturnedData} from './constant-returned-data-v1.mjs';
export function nativeReadFeedback(session,path,code,events,stats){
  const result=previousFeedback(session,path,code,events,stats);
  // Missing or malformed runtime evidence is already refused upstream. Do not
  // require a source program or a live revision API to preserve that refusal.
  if(result.observationCoverage?.status==='unavailable')return {...result,readObservations:[],returnedFieldGuidance:{complete:false,scope:'constant own-field normal body returns only',effects:'open',reactiveIntent:'open'}};
  const state=session.get(path,code),notes=[],suppressed=[...result.suppressed];
  for(const note of result.notes){
    const registration=note.callbackRegistration;
    const model=registration?.definition?.kind==='source-async-callback-entry'&&registration.returnedKind==='object'&&constantReturnedData(state.program,registration.definition.function);
    const consumption=model&&memoResultFieldUses(state.program,state.source,note.witness,model.data.fields);
    if(model&&consumption)suppressed.push({reason:'exact registered callback returns constant own data fields on every normal body return; keep the read observation without stale-result guidance',model,consumption,observation:note,
      scope:'returned-field-value-guidance-only',effects:'open',reactiveIntent:'open',authority:false,certification:false});
    else notes.push(note);
  }
  if(!session.acceptRevision(result.revision).valid||JSON.stringify(state.revision)!==JSON.stringify(result.revision))return {...result,notes:[],suppressed:[],acceptedEvents:0,open:[...result.open,{reason:'input revision changed during returned-data projection',authority:false,certification:false}]};
  return {...result,notes,suppressed,readObservations:result.notes,returnedFieldGuidance:{complete:false,scope:'constant own-field normal body returns only',effects:'open',reactiveIntent:'open'}};
}


