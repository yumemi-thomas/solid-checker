import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Positive: rendered by a tag in another file, through a named import.
export function Remote() {
  const value = count();
  return <b>{value}</b>;
}

// Positive: rendered by a dotted tag through a namespace import
// (`<Parts.Chip />`), whose final property is the reference.
export function Chip() {
  const value = count();
  return <b>{value}</b>;
}
