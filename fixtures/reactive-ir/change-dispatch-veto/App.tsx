import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A `click()` in a component's body delivers the checkbox's `change` inside that body's owned scope (Chrome: `change` fires in the call).
export function Dispatching(props: { box: HTMLInputElement }) {
  props.box.click();
  return <p />;
}
