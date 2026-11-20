import { createEffect, createStore } from "solid-js";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A function of this program that shares a runtime export's name is no import of it, and it writes no `type`: the proof is not refused by the name.
function setProperty(counter: { count: number }, value: number) {
  counter.count = value;
}

export function bump(counter: { count: number }) {
  setProperty(counter, 1);
}
