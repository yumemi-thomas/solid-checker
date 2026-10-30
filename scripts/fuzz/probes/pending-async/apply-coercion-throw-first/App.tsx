// ADR 0507 residual (ADR 0511): a coercion in the apply before the read throws first, so the run never reaches the pending read.
import { createMemo, createRenderEffect } from "solid-js";

const bad = { toString(): string { throw new Error("coerced"); } };

export function C1() {
  const a1 = createMemo(async () => 1);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    const label = "x" + bad;
    a1();
  });
  return (
    <div>
    </div>
  );
}
