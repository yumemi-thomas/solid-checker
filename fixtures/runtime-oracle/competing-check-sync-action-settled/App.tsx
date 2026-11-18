import { action, createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  const a1 = action(function* () {
    createMemo(async () => 1, { sync: true });
  });
  onSettled(() => {
    a1();
  });
  return (
    <div>
    </div>
  );
}
