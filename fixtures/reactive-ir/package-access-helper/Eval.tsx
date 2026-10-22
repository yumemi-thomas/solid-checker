import { createSignal } from "solid-js";
import { access } from "@solid-primitives/utils";

// Uncertifiable: a direct `eval` reaches the binding with no static
// reference, so the file proves nothing about the getter's `length`.
export function UncertifiableEval() {
  const [value] = createSignal(1);
  eval("Object.defineProperty(value, 'length', { value: 1 })");
  const initial = access(value);
  return <span>{initial}</span>;
}
