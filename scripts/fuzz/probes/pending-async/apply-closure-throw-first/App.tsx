// ADR 0510 (false positive of ADR 0504): the closure the apply calls throws before its read of the pending memo; rc.17 reports
// no PENDING_ASYNC_UNTRACKED_READ.
import { createMemo, createRenderEffect } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    const f1 = () => {
      JSON.parse("{");
      a1();
    };
    f1();
  });
  return (
    <div>
    </div>
  );
}
