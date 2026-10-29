// ADR 0510 review: the body reads a member of an `any` that is undefined before the effect is created, so it throws a
// TypeError first; the apply never runs and nothing reads the module-scope memo pending.
import { createMemo, createRenderEffect } from "solid-js";

const a1 = createMemo(async () => 1);

export function C1(props: { cfg?: any }) {
  const v1 = props.cfg.x;
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    a1();
  });
  return (
    <div>
      {String(v1)}
    </div>
  );
}
