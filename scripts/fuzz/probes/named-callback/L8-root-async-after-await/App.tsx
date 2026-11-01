import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = async () => { await Promise.resolve(); const f1 = () => { s1(); }; f1(); };
  createRoot(cb1);
  return <div></div>;
}

export default C1;
