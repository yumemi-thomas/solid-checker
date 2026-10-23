// An `action(...)` call withholds the proof wherever it is written. Its
// invocation flushes on the invoker's stack for a synchronous `yield`
// (`@solidjs/signals` rc.17 `dist/dev.js:1455-1465` `inTransaction`, called at
// `:1636`), and the invocation cannot be placed from the call that creates the
// wrapper, so even a module-level creation counts.
import { action, createEffect, createMemo, createSignal } from "solid-js";

const [v, setV] = createSignal(1);
export const save = action(function* () {
  setV(2);
  yield;
});

createEffect(v, () => {
  createMemo(() => 1);
});
