// A direct `eval` in the file can write the holder (`eval("box.m = f")`) with
// no reference the census sees, so the boxed member names nothing.
import { createMemo, createRenderEffect, refresh } from "solid-js";

const refreshBox = { m: refresh };
const refreshBoxed = refreshBox.m;
eval("1");

export function EvalBox() {
  const m1 = createMemo(() => 1);
  createRenderEffect(() => 1, () => {
    refreshBoxed(m1);
  });
  return <div />;
}
