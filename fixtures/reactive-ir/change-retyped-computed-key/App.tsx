import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A write under a key nothing proves: it may be `type`.
export function put(el: HTMLElement, key: string) {
  (el as unknown as Record<string, unknown>)[key] = "text";
}
