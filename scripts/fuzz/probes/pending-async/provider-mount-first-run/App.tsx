// ADR 0510: a child one context provider renders mounts during its inert parent's tree; its render effect's first apply runs on
// the call and meets the child's own async memo pending: PENDING_ASYNC_UNTRACKED_READ.
import { createContext, createMemo, createRenderEffect, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

const ValCtx = createContext<{ r1: Accessor<number> }>();

function C1() {
  const a1 = createMemo(async () => 1);
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

export function P1() {
  const [r1] = createSignal(1);
  return (
    <div>
      <ValCtx value={{ r1 }}>
        <C1 />
      </ValCtx>
    </div>
  );
}
