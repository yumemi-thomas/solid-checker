// ADR 0507 residual distinction (ADR 0511): the coerced `toString` returns, so the first run meets the memo pending and rc.17
// throws PENDING_ASYNC_UNTRACKED_READ; the checker still refuses the operator (uncertifiable, a precision cost, not a claim).
import { createMemo, createRenderEffect } from "solid-js";

const bad = { toString(): string { return "ok"; } };

export function C1() {
  const a1 = createMemo(async () => 1);
  const label = "x" + bad;
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
