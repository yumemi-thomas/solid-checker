import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => { const f1 = () => { s1(); }; f1(); };
  return <div><button onClick={() => createRoot(cb1)}>a</button></div>;
}

export default C1;
