import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { const f1 = () => { s1(); }; return f1; };
  const g = createRoot(cb1);
  return <div><button onClick={g}>a</button></div>;
}

export default C1;
