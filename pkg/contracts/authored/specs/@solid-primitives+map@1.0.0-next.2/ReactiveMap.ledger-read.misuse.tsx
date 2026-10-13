import { createMemo } from "solid-js";
import { ReactiveMap } from "@solid-primitives/map";
export default function App() {
  const cache = new ReactiveMap([["a", 1]]);
  const current = cache.get("a");
  void current;
  return document.createElement("p");
}
