// Positive: the eval-free control of EvalReassignedComponent.tsx. The
// component's binding is never written, so its owned-scope write is proven.
import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);
export function Control() {
  setCount(1);
  return <button />;
}
