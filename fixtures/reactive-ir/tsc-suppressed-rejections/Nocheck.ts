// @ts-nocheck
import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// The leading pragma switches checking off: nothing reports the call.
export function unchecked() {
  createEffect(() => count());
}
