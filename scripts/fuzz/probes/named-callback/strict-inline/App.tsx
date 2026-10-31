import { createSignal, createRoot } from "solid-js";
export function C1() {
  const [s1] = createSignal(0);
  createRoot(() => {
    const f1 = () => { s1(); };
    f1();
  });
  return <div></div>;
}

export default C1;
