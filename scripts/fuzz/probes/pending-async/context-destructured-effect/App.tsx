// ADR 0511: the context's memo is taken out of a local object literal (`const { read } = { read: ctx.a1 }`) before a user
// effect; the first render's tail flush runs the apply while the memo is pending: rc.17 throws PENDING_ASYNC_UNTRACKED_READ.
import { createContext, createEffect, createMemo, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const ValCtx = createContext<{ a1: Accessor<number> }>();

function C1() {
  const ctx = useContext(ValCtx);
  const { read } = { read: ctx.a1 };
  createEffect(() => {
    return 1;
  }, (v) => {
    read();
  });
  return <div></div>;
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
