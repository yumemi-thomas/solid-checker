// ADR 0511 negative: the hook throws a SyntaxError after creating the memo, so the body ends before the effect exists.
// Nothing reads the memo.
import { createRenderEffect } from "solid-js";
import { useVals } from "./helpers";

export function C1() {
  const { a1 } = useVals();
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
