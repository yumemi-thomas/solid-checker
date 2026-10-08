import { createMemo } from "solid-js";
import { ReactiveMap } from "@solid-primitives/map";
export default function App() {
  const cache = new ReactiveMap([["a", 1]]);
  createMemo(() => cache.get("a"));
  return document.createElement("p");
}
