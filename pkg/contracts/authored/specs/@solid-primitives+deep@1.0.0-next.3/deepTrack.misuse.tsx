import { createMemo, createStore } from "solid-js";
import { deepTrack } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({ count: 0 });
  const current = deepTrack(store).count;
  void current;
  return document.createElement("p");
}
