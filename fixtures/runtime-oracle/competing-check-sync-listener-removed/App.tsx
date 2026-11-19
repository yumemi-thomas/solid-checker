import { createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  const el1 = document.createElement("input");
  document.body.append(el1);
  el1.addEventListener("blur", () => {
    createMemo(async () => 1, { sync: true });
  });
  el1.focus();
  onSettled(() => {
    el1.remove();
  });
  return (
    <div>
    </div>
  );
}

