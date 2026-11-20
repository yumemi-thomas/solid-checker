import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// The attribute API reached by a folded key: the name is spelled nowhere, and the key's proof names it.
export function retype(el: HTMLInputElement) {
  (el as any)["set" + "Attribute"]("type", "text");
}
