import { createRoot, createSignal, resolve } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { const f1 = async () => { await Promise.resolve(); s1(); }; f1(); };
  createRoot(cb1);
  return <div></div>;
}

export default C1;
