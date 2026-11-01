import { createSignal, untrack } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  const cb1 = () => { const f1 = () => { setW1(7); }; return f1; };
  const g = untrack(cb1);
  g();
  return <div></div>;
}

export default C1;
