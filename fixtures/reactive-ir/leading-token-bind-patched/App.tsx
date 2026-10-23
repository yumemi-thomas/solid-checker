import { createEffect, createReaction, createSignal } from "solid-js";

const [count] = createSignal(0);

// The project replaces `Function.prototype.bind` (ADR 0338's tripwire), so a
// bound call is not proven to run its target.
const nativeBind = Function.prototype.bind;
Function.prototype.bind = function (this: Function, thisArg: unknown, ...args: unknown[]) {
  return nativeBind.call(this, thisArg, ...args);
};

// NO VIOLATION: `setupBound.bind(null)()` may not run `setupBound`.
function setupBound() {
  createEffect(count, () => {});
}
setupBound.bind(null)();

// NO VIOLATION: `trackBound.bind(null)(…)` may not arm the reaction.
const trackBound = createReaction(() => {
  createEffect(count, () => {});
});
trackBound.bind(null)(() => count());

// VIOLATION: `call` is not `bind`; `setupCalled.call(null)` runs it unowned.
function setupCalled() {
  createEffect(count, () => {});
}
setupCalled.call(null);
