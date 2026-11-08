import { createSignal, getOwner } from "solid-js";
export const [w, setW] = createSignal(0);
let runs = 0;
export default function Hidden() { runs += 1; (window as any).__log?.("Hidden", "body " + (getOwner() ? "owner" : "no-owner") + " runs=" + runs + " w=" + w()); return <i>h</i>; }
