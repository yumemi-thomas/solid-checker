import { createSignal, onCleanup } from "solid-js";
const [count] = createSignal(1);
export const timer = { useTicker() { onCleanup(() => {}); } };
export const reader = { readCount() { return count(); } };
export const callbacks = { invoke(fn: () => void) { fn(); } };
