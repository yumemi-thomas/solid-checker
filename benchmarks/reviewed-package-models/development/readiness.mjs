// Explicit authored read of the same memo/field already used by the view.
// No Promise patching, timer quiet period, or dynamic settle-effect ownership.
export function completionProtocol(h,Solid){
  h.checkPending=Solid.isPending;
}
export async function waitForApplication(page,kind){
  if(kind==='authored-pending-probe'){
    await page.waitForFunction(()=>typeof globalThis.__experiment?.probePending==='function'&&!globalThis.__experiment.probePending());
  }
  else if(kind==='authored-body-counter'){
    // Intentionally pending fixtures: observe an authored body phase. This is
    // neither a Promise settlement claim nor a generic browser idle detector.
    await page.waitForFunction(()=>globalThis.__experiment?.values.calls>=1);
  }else throw Error('No admitted application completion protocol');
}
