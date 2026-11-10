import { createEffect, createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  const m1 = createMemo(async () => {
    await Promise.resolve();
    onSettled.call(null, () => () => {});
    return 1;
  });
  createEffect(() => {
    return m1();
  }, (v) => {});
  return (
    <div>
    </div>
  );
}
