// ADR 0510 (false positive of ADR 0504): the IIFE's JSON.parse throws a SyntaxError before the read of the pending memo, so
// rc.17 never reports PENDING_ASYNC_UNTRACKED_READ; the read is an obligation.
import { createMemo, createRenderEffect } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    (() => {
      JSON.parse("{");
      a1();
    })();
  });
  return (
    <div>
    </div>
  );
}
