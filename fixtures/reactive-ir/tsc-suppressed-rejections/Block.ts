import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// A directive over two lines: tsc 5 files it under its last line, tsgo under
// its first, so only one compiler suppresses the call. Uncertifiable.
export function block() {
  /* @ts-ignore
     two lines */
  createEffect(() => count());
}
