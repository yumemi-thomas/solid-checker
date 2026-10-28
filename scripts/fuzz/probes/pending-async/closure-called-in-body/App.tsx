// ADR 0490 (from the misuse matrix): a read in a component closure called in the body is unsuspendable (PENDING_ASYNC_UNTRACKED_READ).
import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const f1 = () => {
    a1();
  };
  f1();
  return (
    <div>
    </div>
  );
}
