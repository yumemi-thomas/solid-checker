// Fresh renamed source after the final guard-refusal profile freeze.
// This remains an authored retention workload, not a real application defect.
import previous from './bounded-history-cases-v2.mjs';
export default previous.map(row=>({...row,id:row.id.replace('-640-', '-final-640-'),
  source:row.source.replaceAll('refreshCount','phaseCount').replaceAll('setRefreshCount','setPhaseCount'),
  artifactOrigin:'fresh-source-after-final-guard-refusal-profile-freeze'}));
