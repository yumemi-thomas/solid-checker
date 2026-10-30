// ADR 0507 residual (ADR 0511): a coercion in the body before the effect runs a throwing `toString`, so the body throws and
// the render effect never runs; nothing reads the pending memo. `throws_nothing_before` refuses the operator.
import { createMemo, createRenderEffect } from "solid-js";

const bad = { toString(): string { throw new Error("coerced"); } };

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
