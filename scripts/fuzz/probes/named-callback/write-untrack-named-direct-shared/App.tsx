import { createSignal, untrack } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  const cb1 = () => { setW1(7); };
  untrack(cb1);
  untrack(cb1);
  return <div></div>;
}

export default C1;
