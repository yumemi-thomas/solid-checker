// ADR 0496 (from the misuse matrix): a read in a closure called in a createRoot callback the body runs is unsuspendable (PENDING_ASYNC_UNTRACKED_READ).
import { createMemo, createRoot, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  createRoot(() => {
    const f1 = () => {
      a1();
    };
    f1();
  });
  return (
    <div>
    </div>
  );
}
