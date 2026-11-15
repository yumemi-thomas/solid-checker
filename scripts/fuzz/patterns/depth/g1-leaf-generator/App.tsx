import { createTrackedEffect, onCleanup } from "solid-js";
function* g() { onCleanup(() => {}); yield 1; }
export function App() {
  createTrackedEffect(() => { for (const _ of g()) {} });
  return <i>app</i>;
}
