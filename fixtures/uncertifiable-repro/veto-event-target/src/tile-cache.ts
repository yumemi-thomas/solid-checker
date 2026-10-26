// app-game gpu-text-rendering pageTileCache.ts:27 and prepareCurveDocument.ts:163:
// the one synchronous dispatch of this project, on a fresh `new EventTarget()`,
// in code the analysis does not place (app-game: a callback the WebGPU queue's
// work-done promise runs; here an exported function nothing calls). It reaches
// only that object's listeners, never a JSX handler, but no exclusion of
// ADR 0393 covers it, so it withholds every handler's owner and read proofs
// (ADR 0356).
const events = new EventTarget();

export const tileEvents: EventTarget = events;

export function notifyTilesChanged() {
  events.dispatchEvent(new Event("change"));
}
