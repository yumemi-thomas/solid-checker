import { createEffect, createStore } from "solid-js";

// Solid 2 removed the `attr:` namespace and the compiler refuses the file (SC9015): it has no compiler facts, so nothing in it is proven, and the proof never starts.
export function Refused() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" attr:type="text" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}
