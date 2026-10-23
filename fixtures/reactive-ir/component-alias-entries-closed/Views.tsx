import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: `Picker.tsx` reads the namespace by a computed key and
// renders what it gets, an entry the reference census does not show.
export function Picked() {
  const value = count();
  return <b>{value}</b>;
}
