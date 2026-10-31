import { createRoot, createSignal } from "solid-js";
function h1(cb: () => unknown) { cb(); return 1; }
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { h1(() => { s1(); }); };
  createRoot(cb1);
  return <div></div>;
}

export default C1;
