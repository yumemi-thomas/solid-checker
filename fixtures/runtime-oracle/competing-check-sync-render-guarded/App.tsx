import { createMemo, createRenderEffect, createSignal } from "solid-js";

export function C1() {
  const [t1, setT1] = createSignal(1);
  createRenderEffect(() => {
    return t1();
  }, (v) => {
    if (v === 1) return;
    {
      createMemo(async () => 1, { sync: true });
    }
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        setT1(2);
      }}>go</button>
    </div>
  );
}
