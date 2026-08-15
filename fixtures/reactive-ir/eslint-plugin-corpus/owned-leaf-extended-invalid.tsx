import {
  action,
  createMemo,
  createOptimistic,
  createSignal,
  createTrackedEffect,
  flush,
  onCleanup,
  refresh,
  untrack,
} from "solid-js";

const value = createMemo(() => 1);
createMemo(() => refresh(value));

const [, setOptimistic] = createOptimistic(0);
createMemo(() => setOptimistic(1));

const save = action(function* () {});
createMemo(() => save());

createTrackedEffect(() => {
  onCleanup(() => {});
  flush();
});

const [, setSignal] = createSignal(0);
createMemo(() => setSignal(1));
// The rc.0 guard keys on the owner, not on tracking: untrack() inside a memo
// keeps the memo's owner, so the write still throws at runtime.
createMemo(() => untrack(() => setSignal(2)));
