import { createSignal, untrack } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  const f1 = () => { setW1(7); };
  const cb1 = () => { f1(); };
  untrack(cb1); untrack(cb1);
  return <div></div>;
}

export default C1;
