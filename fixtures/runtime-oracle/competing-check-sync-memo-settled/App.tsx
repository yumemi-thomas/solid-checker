import { createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  onSettled(() => {
    createMemo(async () => 1, { sync: true });
  });
  return (
    <div>
    </div>
  );
}
