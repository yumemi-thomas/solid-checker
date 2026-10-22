import { createSignal } from "solid-js";
import { access } from "@solid-primitives/utils";

// Uncertifiable: an exported getter is reachable through the module namespace
// (`exported-mut.ts` walks it with `Object.values` and redefines `length`
// before any component renders), so its arity is not proven here.
export const [value] = createSignal(1);

export function UncertifiableModuleExport() {
  const initial = access(value);
  return <span>{initial}</span>;
}
