import { createEffect, createStore } from "solid-js";
import { setProperty } from "@solidjs/web";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// The web runtime's own writer, called with a name the caller gives: it retypes with no `type` write in the file.
export function retype(el: HTMLInputElement) {
  setProperty(el, "type", "text");
}
