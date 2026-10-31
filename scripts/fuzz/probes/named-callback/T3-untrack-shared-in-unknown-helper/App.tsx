import { createSignal, untrack } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  const cb1 = () => { const f1 = () => { setW1(7); }; f1(); };
  const run = () => { untrack(cb1); untrack(cb1); };
  return <div><button onClick={run}>a</button></div>;
}

export default C1;
