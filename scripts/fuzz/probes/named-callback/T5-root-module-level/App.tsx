import { createRoot, createSignal } from "solid-js";
const [s1] = createSignal(0);
const cb1 = () => { const f1 = () => { s1(); }; f1(); };
createRoot(cb1);
export function C1() { return <div></div>; }

export default C1;
