// Fuzzer source axis (false clean): a child assigns to a store read from its parent's context, outside the setter; Solid reports STORE_WRITE_OUTSIDE_SETTER at the assignment.
import { createContext, createStore, useContext } from "solid-js";
import type { StoreSetter } from "solid-js";

type Vals = { s1: { v: number }; setS1: StoreSetter<{ v: number }> };
const ValCtx = createContext<Vals>();

function C1() {
  const { s1 } = useContext(ValCtx);
  s1.v = 7;
  return <div>{s1.v}</div>;
}

export function P1() {
  const [s1, setS1] = createStore({ v: 0 });
  return (
    <ValCtx value={{ s1, setS1 }}>
      <C1 />
    </ValCtx>
  );
}
