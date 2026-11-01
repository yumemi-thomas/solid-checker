import { createSignal, untrack, action } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  const a1 = action(function* () { setW1(7); });
  const cb1 = () => { const f1 = () => { a1(); }; f1(); };
  untrack(cb1);
  return <div></div>;
}

export default C1;
