import { createMemo, createRenderEffect, createSignal, createStore } from "solid-js";

export function C1() {
  const [s1, setS1] = createStore({ v: 0 });
  const [t1, setT1] = createSignal(1);
  createRenderEffect(() => {
    return t1();
  }, (v) => {
    if (v === 1) return;
    {
      { const f = () => s1.v; createMemo(f); f(); };
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
