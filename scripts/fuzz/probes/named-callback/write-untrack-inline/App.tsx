import { createSignal, untrack } from "solid-js";
export function C1() {
  const [w1, setW1] = createSignal(0);
  untrack(() => {
    const f1 = () => { setW1(7); };
    f1();
  });
  return <div></div>;
}

export default C1;
