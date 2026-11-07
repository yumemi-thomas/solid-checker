import { createSignal } from "solid-js";
import { readsWithEval } from "./eval-helper";

// The eval case of ../../App.tsx, in a project of its own: a direct `eval`
// in the project may patch array iteration (ADR 0803), so every tuple slot
// here is withdrawn as well.
export function Eval() {
  const [s] = createSignal(1);
  const value = readsWithEval({ get a() { return s(); } });
  return <main>{value}</main>;
}
