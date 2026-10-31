import { createSignal, runWithOwner, getOwner } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  const o = getOwner();
  const cb1 = () => { const f1 = () => { s1(); }; f1(); };
  runWithOwner(o, cb1);
  return <div></div>;
}

export default C1;
