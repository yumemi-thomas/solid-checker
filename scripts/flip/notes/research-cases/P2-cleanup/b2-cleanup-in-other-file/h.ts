import { createSignal, onCleanup } from "solid-js";
const [c] = createSignal(1);
export function useX() { onCleanup(() => { c(); }); }
