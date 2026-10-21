import { createReaction, createSignal, onCleanup } from "solid-js";

const [count] = createSignal(0);
declare function schedule(fn: () => void): void;
let stored: (() => void) | undefined;

// A wrapper that stores its callback runs it later, out of band, under whatever
// owner that run finds. The return is the callback, but the parameter is also
// stored, so the wrapper is no identity.
function storeWrap(callback: () => void): () => void {
  stored = callback;
  return callback;
}
const trackStored = createReaction(storeWrap(() => onCleanup(() => {})));
trackStored(() => count());

// A wrapper that hands its callback to a scheduler is no identity either.
function scheduleWrap(callback: () => void): () => void {
  schedule(callback);
  return callback;
}
const trackScheduled = createReaction(scheduleWrap(() => onCleanup(() => {})));
trackScheduled(() => count());

// A wrapper that returns another function on one path.
function otherWrap(callback: () => void): () => void {
  if (stored) {
    return () => {};
  }
  return callback;
}
const trackOther = createReaction(otherWrap(() => onCleanup(() => {})));
trackOther(() => count());

// Both paths return the callback, but no straight-line return proves that every
// completion does, so the wrapper is not an identity (fail closed).
function branchWrap(callback: () => void): () => void {
  if (stored) {
    return callback;
  } else {
    return callback;
  }
}
const trackBranch = createReaction(branchWrap(() => onCleanup(() => {})));
trackBranch(() => count());

// A declaration without a body is unresolved here: nothing proves what it
// returns.
declare function unresolvedWrap(callback: () => void): () => void;
const trackUnresolved = createReaction(unresolvedWrap(() => onCleanup(() => {})));
trackUnresolved(() => count());

// A member callee is not a plain identifier, so it names no wrapper.
const helpers = {
  wrap: (callback: () => void): (() => void) => callback,
};
const trackMember = createReaction(helpers.wrap(() => onCleanup(() => {})));
trackMember(() => count());

// A local `wrapReaction` shadows the module's identity wrapper of the same
// name: the call resolves to the local symbol, which returns a different
// function.
function wrapReaction(callback: () => void): () => void {
  return callback;
}
export function Shadowed() {
  const wrapReaction = (_callback: () => void): (() => void) => () => {};
  const trackShadowed = createReaction(wrapReaction(() => onCleanup(() => {})));
  trackShadowed(() => count());
  return wrapReaction;
}
