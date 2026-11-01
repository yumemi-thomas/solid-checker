import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const wrapUse = () => { s1(); };
  const cb1 = () => { wrapUse(); };
  createRoot(cb1);
  return <div></div>;
}

export default C1;
