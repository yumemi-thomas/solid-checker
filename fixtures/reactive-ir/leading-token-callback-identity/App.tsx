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
