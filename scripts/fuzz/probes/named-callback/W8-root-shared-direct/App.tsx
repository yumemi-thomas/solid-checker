import { createRoot, createSignal } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  const cb1 = () => { setW1(7); };
  createRoot(cb1); createRoot(cb1);
  return <div></div>;
}

export default C1;
