// ADR 0504 review (false positive): the apply returns on its first run, whose value is the compute's 1, so the read of the pending memo never runs; rc.17 reports nothing.
import { createEffect, createMemo } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  createEffect(() => {
    return 1;
  }, (v) => {
    if (v === 1) return;
    a1();
  });
  return (
    <div>
    </div>
  );
}
