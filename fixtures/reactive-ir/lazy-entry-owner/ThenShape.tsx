import { onCleanup } from "solid-js";

// The loader is `import(…).then((m) => ({ default: m.Name }))`, the shape the corpus writes (viviana-ui-web, solid-groove): not the literal `() => import("…")` the rule takes. Open.
export function ThenShape() {
  onCleanup(() => {});
  return <h6>then</h6>;
}
