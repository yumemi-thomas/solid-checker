import { createSignal } from "solid-js";

const [count] = createSignal(0);

// No violation: `Swapped` is written before any render, so the call and the
// alias tag both run the replacement; the original arrow runs nowhere. Its
// read is uncertifiable at most (a written binding is no exact alias).
let Swapped = () => {
  const value = count();
  return <b>{value}</b>;
};
Swapped = () => <i />;

const S2 = Swapped;

export function SwappedHost() {
  return <p>{Swapped()}<S2 /></p>;
}
