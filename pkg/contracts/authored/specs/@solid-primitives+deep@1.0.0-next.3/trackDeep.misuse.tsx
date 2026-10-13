import { createMemo, createStore } from "solid-js";
import { trackDeep } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({ count: 0 });
  const current = trackDeep(store).count;
  void current;
  return document.createElement("p");
}
