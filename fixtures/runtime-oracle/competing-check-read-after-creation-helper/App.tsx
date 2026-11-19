import { createEffect, createMemo, createSignal, createStore } from "solid-js";

export function C1() {
  const [s1, setS1] = createStore({ v: 0 });
  const wrapUse = () => { { const f = () => s1.v; createMemo(f); f(); }; };
  createEffect(() => {
    return 1;
  }, (v) => {
    wrapUse();
  });
  return (
    <div>
    </div>
  );
}
