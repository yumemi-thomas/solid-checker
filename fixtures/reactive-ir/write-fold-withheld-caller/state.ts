import { createSignal } from "solid-js";

export const [s, setS] = createSignal(0);
export function sync() { setS(1); }
