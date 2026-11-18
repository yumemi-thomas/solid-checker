import { action, createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = action(function* () {
    createMemo(async () => 1, { sync: true });
  });
  createEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return (
    <div>
    </div>
  );
}
