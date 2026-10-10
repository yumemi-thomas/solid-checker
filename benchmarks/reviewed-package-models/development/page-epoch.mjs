// Retry only an interrupted execution context, from a fresh app state. Semantic
// errors still stop immediately. Never combine snapshots across page loads.
export async function inPageEpoch(page,work,{attempts=3,onRetry=()=>{}}={}){
  for(let attempt=1;attempt<=attempts;attempt++){
    try{return await work();}
    catch(error){
      const interrupted=error.code==='APP_PAGE_EPOCH_CHANGED'||/Execution context was destroyed|Cannot find context with specified id/.test(error.message);
      if(!interrupted||attempt===attempts)throw error;
      onRetry({attempt,reason:error.message});await page.reload({waitUntil:'domcontentloaded'});
    }
  }
}
export function requirePageEpoch(snapshot,loadId){
  if(!snapshot||snapshot.loadId!==loadId){const error=Error('Application page load changed during observation');error.code='APP_PAGE_EPOCH_CHANGED';throw error;}
  return snapshot;
}
