// Fresh source variant after the final bounded profile freeze. The older
// preparation remains an adapted population; no application defect is claimed.
import earlier from './bounded-history-cases-v1.mjs';
export default earlier.map(row=>({...row,id:row.id.replace('512','640'),
  source:row.source.replaceAll('512','640').replaceAll('pulse','refreshCount').replaceAll('setPulse','setRefreshCount'),
  artifactOrigin:'fresh-640-cycle-retention-challenge-after-final-profile-freeze'}));
