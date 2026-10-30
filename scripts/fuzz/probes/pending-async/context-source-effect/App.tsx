// ADR 0510: the mounting parent provides its async memo through one context provider; the child's user effect runs in the first
// render's tail flush, which nothing holds, and meets it pending: rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createContext, createEffect, createMemo, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const ValCtx = createContext<{ a1: Accessor<number> }>();

function C1() {
  const ctx = useContext(ValCtx);
  createEffect(() => {
    return 1;
  }, (v) => {
    ctx.a1();
  });
  return (
    <div>
    </div>
  );
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <ValCtx value={{ a1 }}>
        <C1 />
      </ValCtx>
    </div>
  );
}
