// Fuzzer source axis (false clean): an accessor from a context, aliased and called in the body; Solid reports STRICT_READ_UNTRACKED (the checker reports `ctx.r1()` called directly, not the alias).
import { createContext, createSignal, useContext } from "solid-js";
import type { Accessor } from "solid-js";

type Vals = { r1: Accessor<number> };
const ValCtx = createContext<Vals>();

function C1() {
  const ctx = useContext(ValCtx);
  const useIt = ctx.r1;
  const v = useIt();
  return <div>{v}</div>;
}

export function P1() {
  const [r1] = createSignal(3);
  return (
    <ValCtx value={{ r1 }}>
      <C1 />
    </ValCtx>
  );
}
