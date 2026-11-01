import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const cb1 = () => <p>{s1()}</p>;
  const v = createRoot(cb1);
  return <div>{v}</div>;
}

export default C1;
