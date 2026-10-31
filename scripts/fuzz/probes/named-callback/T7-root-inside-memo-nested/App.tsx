import { createMemo, createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { const f1 = () => { s1(); }; f1(); };
  const m = createMemo(() => { createRoot(cb1); return 1; });
  return <div>{m()}</div>;
}

export default C1;
