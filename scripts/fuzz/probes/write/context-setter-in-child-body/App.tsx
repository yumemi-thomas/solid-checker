// Fuzzer source axis (false clean): a child reads a setter from a context its parent provides and calls it in its body; Solid reports REACTIVE_WRITE_IN_OWNED_SCOPE at the call.
import { createContext, createSignal, useContext } from "solid-js";
import type { Accessor, Setter } from "solid-js";

type Vals = { w1: Accessor<number>; setW1: Setter<number> };
const ValCtx = createContext<Vals>();

function C1() {
  const ctx = useContext(ValCtx);
  ctx.setW1(7);
  return <div>{ctx.w1()}</div>;
}

export function P1() {
  const [w1, setW1] = createSignal(0);
  return (
    <ValCtx value={{ w1, setW1 }}>
      <C1 />
    </ValCtx>
  );
}
