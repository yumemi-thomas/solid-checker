import { createMemo, createStore } from "solid-js";
import { trackStore } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({ count: 0 });
  const current = trackStore(store).count;
  void current;
  return document.createElement("p");
}
