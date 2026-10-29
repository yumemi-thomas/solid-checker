// ADR 0504 (from the misuse matrix): a render effect's first run, on the call under a module-scope mount, reads an async memo created earlier in the body while it is pending (PENDING_ASYNC_UNTRACKED_READ).
import { createMemo, createRenderEffect, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return (
    <div>
    </div>
  );
}
