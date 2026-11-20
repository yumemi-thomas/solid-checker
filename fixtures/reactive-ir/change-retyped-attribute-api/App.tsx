import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A name of the DOM's attribute API: `setAttribute("type", …)` and `removeAttribute("type")` retype a control.
export function retype(el: HTMLElement) {
  el.setAttribute("type", "text");
}
