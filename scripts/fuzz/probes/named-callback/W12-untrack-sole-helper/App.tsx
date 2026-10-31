import { createSignal, untrack } from "solid-js";
function h1(cb: () => unknown) { cb(); return 1; }
export function C1() {
  const [w1, setW1] = createSignal(0);
  const cb1 = () => { h1(() => { setW1(7); }); };
  untrack(cb1);
  return <div></div>;
}

export default C1;
