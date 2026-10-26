// app-game apps/web's two kinds of synchronous dispatch, which together hold
// every handler proof of that project (ADR 0356); excluding one kind moves
// nothing while the other stays.
// Event target: gpu-text-rendering pageTileCache.ts:27, prepareCurveDocument.ts:163,
// a fresh `new EventTarget()` dispatched in code the analysis does not place
// (app-game: a WebGPU work-done callback; here an exported function nothing
// calls). It reaches only that object's listeners, but no exclusion of ADR
// 0435 covers it.
const events = new EventTarget();

export const tileEvents: EventTarget = events;

export function notifyTilesChanged() {
  events.dispatchEvent(new Event("change"));
}

// Helper: ui-components-examples cursor-emulation/eventDispatcher.ts:168,189,
// an element dispatch in a module helper that a returned object's method
// calls, so nothing places the caller.
function dispatchPointer(target: Element, type: string) {
  target.dispatchEvent(new PointerEvent(type, { bubbles: true }));
}

export function createEventDispatcher() {
  return {
    press(target: Element) {
      dispatchPointer(target, "pointerdown");
    },
  };
}
