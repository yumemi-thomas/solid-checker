// ADR 0510 review: the apply reads a member of undefined before the read, so its first run throws a TypeError first.
import { createMemo, createRenderEffect } from "solid-js";

const a1 = createMemo(async () => 1);

export function C1() {
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    (v as any).a.b;
    a1();
  });
  return (
    <div>
    </div>
  );
}
