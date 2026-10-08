import { createMemo, createStore } from "solid-js";
import { deepTrack } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({ count: 0 });
  createMemo(() => deepTrack(store).count);
  return document.createElement("p");
}
