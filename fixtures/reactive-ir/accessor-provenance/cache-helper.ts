import { createSignal } from "solid-js";
const [count] = createSignal(1);
function plain() {}
export function helper() { void count(); }
