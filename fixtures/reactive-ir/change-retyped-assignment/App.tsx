import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A write whose key is `type`: the control may be a text control by the time it is focused.
export function retype(el: HTMLInputElement) {
  el.type = "text";
}
