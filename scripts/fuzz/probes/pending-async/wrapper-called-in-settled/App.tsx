// ADR 0490 (from the misuse matrix): a read in a wrapper called through a closure in onSettled is in a forbidden scope (PENDING_ASYNC_FORBIDDEN_SCOPE).
import { createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const wrapUse = () => { a1(); };
  onSettled(() => {
    const f1 = () => {
      wrapUse();
    };
    f1();
  });
  return (
    <div>
    </div>
  );
}
