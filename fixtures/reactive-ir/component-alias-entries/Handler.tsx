import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Violation: `<H2 />` renders `Handled` as a component; its handler entry
// does not clear that entry.
function Handled() {
  const value = count();
  return <b>{value}</b>;
}

const H2 = Handled;

export function HandlerHost() {
  return <p>{Handled()}<H2 /><button onClick={Handled} /></p>;
}
