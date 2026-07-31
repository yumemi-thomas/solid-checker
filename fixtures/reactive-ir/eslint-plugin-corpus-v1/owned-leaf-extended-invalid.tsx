import { createMemo, createSignal } from "solid-js";

// Writing a signal from inside a tracked pure computation is a feedback loop.
const [value, setValue] = createSignal(0);
createMemo(() => setValue(value() + 1));

const [, setSignal] = createSignal(0);
const derived = createMemo(() => 1);
createMemo(() => {
  derived();
  setSignal(2);
});
