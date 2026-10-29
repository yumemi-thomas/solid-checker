// ADR 0510 (false positive of ADR 0504): the helper throws before it runs the callback that reads the pending memo; rc.17
// reports no PENDING_ASYNC_UNTRACKED_READ.
import { createMemo, createRenderEffect } from "solid-js";

function h1(cb: () => unknown) {
  JSON.parse("{");
  cb();
  return 1;
}

export function C1() {
  const a1 = createMemo(async () => 1);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    h1(() => {
      a1();
    });
  });
  return (
    <div>
    </div>
  );
}
