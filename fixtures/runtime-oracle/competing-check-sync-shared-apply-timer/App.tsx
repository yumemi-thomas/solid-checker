import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const g1 = () => {
    createMemo(async () => 1, { sync: true });
  };
  createEffect(() => {
    return 1;
  }, (v) => {
    g1();
  });
  setTimeout(() => g1(), 0);
  return (
    <div>
    </div>
  );
}
