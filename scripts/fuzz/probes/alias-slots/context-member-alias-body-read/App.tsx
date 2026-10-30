// ADR 0511: a provided signal taken out of the context value by a member Get (`const state = account.state`) and read in the
// child's body is the provider's signal: rc.17 warns STRICT_READ_UNTRACKED at the read.
import { createContext, createSignal, useContext } from "solid-js";
import type { Accessor } from "solid-js";

const Aliased = createContext<{ state: Accessor<number> }>();

function C1() {
  const account = useContext(Aliased);
  const state = account.state;
  const value = state();
  return <p>{value}</p>;
}

export function P1() {
  const [state] = createSignal(1);
  return (
    <Aliased value={{ state }}>
      <C1 />
    </Aliased>
  );
}
