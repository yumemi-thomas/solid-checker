import { createMemo, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { const f1 = () => s1(); return f1(); };
  const m1 = createMemo(cb1);
  return <div><p>{m1()}</p></div>;
}

export default C1;
