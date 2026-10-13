// Test-assisted debugging facts. A passing replay is not a certified defect
// or a safe automatic repair; unrelated effects and reactive intent stay open.
export function replayAssertionFeedback(rows,assertions){
  const notes=[],open=[],unchanged=[];
  for(const row of rows){
    const expected=assertions.get(row.id);
    if(typeof expected!=='string'){open.push({id:row.id,reason:'no explicit primary-value assertion was provided',authority:false,certification:false});continue;}
    const beforePassed=row.primary.before===expected,afterPassed=row.primary.after===expected;
    if(beforePassed){unchanged.push({id:row.id,assertionPassedBefore:true,assertionPassedAfter:afterPassed,otherVisibleOutputChanged:row.visible?.changed??null});if(!afterPassed||row.visible?.changed)open.push({id:row.id,reason:!afterPassed?'proposed change breaks a previously passing value assertion':'the value assertion already passed while another visible output changed',authority:false,certification:false});continue;}
    if(!afterPassed){open.push({id:row.id,reason:'the proposed change does not satisfy the provided value assertion',authority:false,certification:false});continue;}
    notes.push({id:row.id,code:'REPLAYED_CAPTURE_SATISFIES_VALUE_ASSERTION',severity:'info',category:'intent-open',message:`Capturing this read during the memo made the provided value assertion pass in the isolated replay (${row.primary.before} → ${row.primary.after}, expected ${expected}). Review the change in execution timing and task counts.`,expected,source:row.source,taskExecutions:row.taskExecutions,thenGetterExecutions:row.thenGetterExecutions,repairSafety:'unproved',reactiveIntent:'open',authority:false,certification:false,scope:'only the provided primary-value assertion under the recorded interaction',limitation:'Other assertions, side effects, input values, executions and developer intent are not established by this replay.'});
  }
  return {notes,open,unchanged,authority:false,certification:false};
}
