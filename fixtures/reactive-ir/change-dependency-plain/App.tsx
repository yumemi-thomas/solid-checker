import { createEffect, createStore } from "solid-js";
import { double } from "dep-plain";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A dependency whose source is analysed and writes nothing of the kind leaves the answer to the project: the proof stands.
export const twice = double;
