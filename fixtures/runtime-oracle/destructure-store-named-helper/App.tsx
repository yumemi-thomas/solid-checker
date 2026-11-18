import { createStore } from "solid-js";

export function C1() {
  const [st] = createStore({ v: 0 });
  const dir1 = () => {
    const { v } = st;
    v;
  };
  dir1();
  return <i />;
}
