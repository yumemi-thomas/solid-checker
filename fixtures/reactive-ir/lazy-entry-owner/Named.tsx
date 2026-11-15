import { onCleanup } from "solid-js";

// Loaded by `lazy(() => import("./Named"), { export: "Named" })`: the export the options name is the one rendered.
export function Named() {
  onCleanup(() => {});
  return <h2>named</h2>;
}
