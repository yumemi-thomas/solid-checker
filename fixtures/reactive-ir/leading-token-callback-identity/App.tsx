import { createEffect, createMemo, createReaction, createSignal } from "solid-js";
import { render } from "@solidjs/web";

const [count, setCount] = createSignal(0);

// CLEAN: `makeCompute()` runs in the click handler, where a write is allowed.
// Its leading token named `makeCompute` the memo's tracked callback.
function makeCompute(): () => number {
  setCount(1);
  return () => 1;
}
function TrackedFactoryCall() {
  return <button onClick={() => { createMemo(makeCompute()); }} />;
}

// VIOLATION: the memo's tracked callback is `computeWrites` itself.
function computeWrites(): number {
  setCount(1);
  return 1;
}
function TrackedIdentifier() {
  return <button onClick={() => { createMemo(computeWrites); }} />;
}

// CLEAN: `makeApply()` runs in the click handler, not as the apply callback.
function makeApply(): (value: number) => void {
  void count();
  return () => {};
}
function ApplyFactoryCall() {
  return <button onClick={() => { createEffect(count, makeApply()); }} />;
}

// VIOLATION: the apply callback is `applyReads` itself.
function applyReads(_value: number): void {
  void count();
}
function ApplyIdentifier() {
  return <button onClick={() => { createEffect(count, applyReads); }} />;
}

render(() => (
  <div>
    <TrackedFactoryCall />
    <TrackedIdentifier />
    <ApplyFactoryCall />
    <ApplyIdentifier />
  </div>
), document.body);

// CLEAN: `setupMember.child.call(null)` runs `child`, never `setupMember`.
function setupMember() {
  createEffect(count, () => {});
}
setupMember.child = () => {};
setupMember.child.call(null);

// VIOLATION: `setupCalled.call(null)` runs `setupCalled` with no owner.
function setupCalled() {
  createEffect(count, () => {});
}
setupCalled.call(null);

// CLEAN: `trackMember.toString()` never calls the tracker.
const trackMember = createReaction(() => {
  createEffect(count, () => {});
});
void trackMember.toString();

// VIOLATION: `trackCalled(…)` arms the reaction from module scope.
const trackCalled = createReaction(() => {
  createEffect(count, () => {});
});
trackCalled(() => count());

// VIOLATION: `trackCall.call(null, …)` and `trackApply.apply(null, […])` are
// the calls `trackCall(…)` and `trackApply(…)`.
const trackCall = createReaction(() => {
  createEffect(count, () => {});
});
trackCall.call(null, () => count());
const trackApply = createReaction(() => {
  createEffect(count, () => {});
});
trackApply.apply(null, [() => count()]);

// VIOLATION: a standard `bind` result runs its target, called directly or
// through a `const` alias.
const trackBound = createReaction(() => {
  createEffect(count, () => {});
});
trackBound.bind(null)(() => count());
const trackAliased = createReaction(() => {
  createEffect(count, () => {});
});
const trackAlias = trackAliased.bind(null);
trackAlias(() => count());
function setupBound() {
  createEffect(count, () => {});
}
setupBound.bind(null)();

// CLEAN: a `bind` result nothing calls runs nothing.
const trackUnbound = createReaction(() => {
  createEffect(count, () => {});
});
void trackUnbound.bind(null);
function setupUnbound() {
  createEffect(count, () => {});
}
void setupUnbound.bind(null);
