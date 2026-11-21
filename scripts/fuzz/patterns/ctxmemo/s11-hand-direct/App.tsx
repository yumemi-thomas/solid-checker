import { createSignal } from "solid-js";
import { hand } from "r1147r";
export function Host() { const [n, setN] = createSignal(0); hand(() => setN(1)); return <span>{n()}</span>; }
