import { createMemo, createSignal } from "solid-js";
import { ReactiveWeakMap } from "@solid-primitives/map";
export default function App() {
  const [value] = createSignal(0);
  const entries = { *[Symbol.iterator]() { value(); } };
  new ReactiveWeakMap(entries);
  return document.createElement("p");
}
