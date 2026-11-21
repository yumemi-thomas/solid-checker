import { createEffect, createSignal, flush } from "solid-js";

// ADR 1163: the compute throws on its first run, which queues the error arm; the body's `flush()` drains it on the body's
// stack, inside the component's strict-read window, so the read warns. ADR 0613's census vetoes: uncertifiable.
export function DrainedInBody() {
  const [count] = createSignal(3);
  createEffect(() => { throw new Error("x"); }, { effect: () => {}, error: () => {
    count();
  } });
  flush();
  return <i>body</i>;
}
