import { createMemo, resolve } from "solid-js";
export function C1() {
  const cb1 = () => {
    const f1 = () => { resolve(() => 1); };
    f1();
    return 1;
  };
  const m1 = createMemo(cb1);
  const m2 = createMemo(cb1);
  return <div><p>{m1()}{m2()}</p></div>;
}

export default C1;
