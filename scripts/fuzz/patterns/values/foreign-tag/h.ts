import { createSignal } from "solid-js";
const [c] = createSignal(1);
export function h() { return c(); }
