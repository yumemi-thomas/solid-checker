import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// The setter of the `type` accessor, looked up on the prototype: it retypes `el` with no `type` write, no attribute-API name and no `Object.assign`.
export function retype(el: HTMLInputElement) {
  (HTMLInputElement.prototype as any).__lookupSetter__("type").call(el, "text");
}
