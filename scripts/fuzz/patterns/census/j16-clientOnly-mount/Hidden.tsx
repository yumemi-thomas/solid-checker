import { createSignal, getOwner } from "solid-js";
const [w, setW] = createSignal(0);
export default function Hidden() { (window as any).__log?.("Hidden", "body " + (getOwner() ? "owner" : "no-owner")); setW(w() + 1); (window as any).__log?.("Hidden", "write ok"); return <i>h</i>; }
