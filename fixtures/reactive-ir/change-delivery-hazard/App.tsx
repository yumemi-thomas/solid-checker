import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// `Object.assign` onto an element: it may write `type`, whatever the literal says (ADR 1125's delivery premise breaks).
export function assign(el: HTMLInputElement, props: object) {
  Object.assign(el, props);
}
