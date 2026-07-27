import {
  createMemo,
  createOptimistic,
  createSignal,
  onSettled,
  refresh,
} from "solid-js";

const [value, setValue] = createSignal(0, { ownedWrite: true });
createMemo(() => setValue(value() + 1));

const [, setOptimistic] = createOptimistic(0, { ownedWrite: true });
createMemo(() => setOptimistic(1));

onSettled(() => {
  refresh(value);
  return () => console.log("clean");
});
