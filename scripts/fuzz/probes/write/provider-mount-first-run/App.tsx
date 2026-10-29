// ADR 0510: a child one context provider renders mounts during its inert parent's tree (the provider's `children` memos are read
// by the parent's insert); the render effect's first apply runs on the call, and its write throws REACTIVE_WRITE_IN_OWNED_SCOPE.
import { createContext, createRenderEffect, createSignal } from "solid-js";
import type { Accessor } from "solid-js";

const ValCtx = createContext<{ r1: Accessor<number> }>();

function C1() {
  const [w1, setW1] = createSignal(0);
  createRenderEffect(() => {
    return 1;
  }, (v) => {
    setW1(7);
  });
  return (
    <div>
      {w1()}
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
