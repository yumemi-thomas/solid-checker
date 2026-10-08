import { createMemo, createSignal } from "solid-js";
import { ReactiveMap } from "@solid-primitives/map";
export default function App() {
  const [value] = createSignal(0);
  const entries = { *[Symbol.iterator]() { value(); } };
  new ReactiveMap(entries);
  return document.createElement("p");
}
