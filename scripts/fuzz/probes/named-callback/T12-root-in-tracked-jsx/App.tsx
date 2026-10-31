import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { const f1 = () => s1(); return f1(); };
  return <div>{createRoot(cb1)}</div>;
}

export default C1;
