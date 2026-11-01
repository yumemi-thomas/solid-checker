import { createMemo } from "solid-js";
export function C1() {
  const a1 = createMemo(async () => 1);
  const cb1 = () => { a1(); };
  return <div></div>;
}

export default C1;
