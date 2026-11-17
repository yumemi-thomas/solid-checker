import { createEffect, createSignal } from "solid-js";

export function C1() {
  const g1 = () => {
    createEffect(() => 1, () => {});
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    g1();
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        g1();
      }}>go</button>
    </div>
  );
}
