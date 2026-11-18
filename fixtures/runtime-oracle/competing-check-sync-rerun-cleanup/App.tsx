import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const [t1, setT1] = createSignal(1);
  createEffect(() => {
    return t1();
  }, (v) => {
    return () => {
      createMemo(async () => 1, { sync: true });
    };
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        setT1(2);
      }}>go</button>
    </div>
  );
}
