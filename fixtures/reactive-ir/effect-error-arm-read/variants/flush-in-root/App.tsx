import { createEffect, createRoot, createSignal, flush } from "solid-js";

// ADR 1163: a `flush()` the program calls drains on its own stack, under its caller's owner and window (`createRoot`'s body
// here, ADR 0613's census vetoes it). The error arm's read may run there, so it stays uncertifiable.
export function DrainedInRoot() {
  const [count] = createSignal(3);
  const [trigger, setTrigger] = createSignal(1);
  createEffect(() => { if (trigger() === 2) throw new Error("x"); return 1; }, { effect: () => {}, error: () => {
    count();
  } });
  return (
    <button onClick={() => {
      setTrigger(2);
      createRoot(() => {
        flush();
      });
    }}>go</button>
  );
}
