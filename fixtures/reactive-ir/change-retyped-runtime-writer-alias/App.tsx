import { createEffect, createStore } from "solid-js";
import { setProperty as sp } from "@solidjs/web";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// The same export under another local name: the import names the export it takes, whatever the alias is.
export function retype(el: HTMLInputElement) {
  sp(el, "type", "text");
}
