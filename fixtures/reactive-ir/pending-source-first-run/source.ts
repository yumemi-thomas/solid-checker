import { createMemo, createSignal } from "solid-js";

// ADR 0510: this module's scope only creates nodes: an async memo of a
// literal (pending until a microtask) and a signal.
export const remote = createMemo(async () => 1);
export const [flag, setFlag] = createSignal(0);
