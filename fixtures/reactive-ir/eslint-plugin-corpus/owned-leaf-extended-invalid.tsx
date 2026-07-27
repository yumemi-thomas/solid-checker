import {
  action,
  createMemo,
  createOptimistic,
  createSignal,
  createTrackedEffect,
  flush,
  onCleanup,
  refresh,
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
createTrackedEffect(() => {
  value();
  setSignal(2);
});
