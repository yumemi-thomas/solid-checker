// Fuzzer async axis (wrong rule): a child reads an async memo from its parent's context in its body while pending; Solid reports PENDING_ASYNC_UNTRACKED_READ, the checker names it STRICT_READ_UNTRACKED (strict-read-untracked).
import { createContext, createMemo, useContext } from "solid-js";
import type { Accessor } from "solid-js";

type Vals = { a1: Accessor<number> };
const ValCtx = createContext<Vals>();

function C1() {
  const ctx = useContext(ValCtx);
  ctx.a1();
  return <div />;
}

export function P1() {
  const a1 = createMemo(async () => 1);
  return (
    <ValCtx value={{ a1 }}>
      <C1 />
    </ValCtx>
  );
}
