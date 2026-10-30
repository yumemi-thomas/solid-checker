// Fuzzer async axis (false clean): onSettled reads an async memo from the parent's context; Solid reports PENDING_ASYNC_FORBIDDEN_SCOPE at the read (with the memo through a prop, or behind a helper, the checker declines instead).
import { createContext, createMemo, createSignal, onSettled, useContext } from "solid-js";
import type { Accessor } from "solid-js";

type C1Vals = { a1: Accessor<number> };
const ValCtx = createContext<C1Vals>();

function C1() {
  const ctx = useContext(ValCtx);
  onSettled(() => {
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
