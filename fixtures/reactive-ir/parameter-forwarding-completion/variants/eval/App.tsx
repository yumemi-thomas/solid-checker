import { createSignal } from "solid-js";
import { useEvaluated } from "./evaluated";

// The eval case of ../../App.tsx, in a project of its own: a direct `eval`
// in the project may patch array iteration (ADR 0803), so every tuple slot
// here is withdrawn as well.
export function Evaluated() {
  const [name] = createSignal("a");
  const label = useEvaluated(name);
  return <p>{label}</p>;
}
