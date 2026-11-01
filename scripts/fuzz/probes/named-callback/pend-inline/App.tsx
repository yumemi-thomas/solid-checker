import { createMemo, createRoot } from "solid-js";
export function C1() {
  const a1 = createMemo(async () => 1);
  createRoot(() => {
    const f1 = () => { a1(); };
    f1();
  });
  return <div></div>;
}

export default C1;
