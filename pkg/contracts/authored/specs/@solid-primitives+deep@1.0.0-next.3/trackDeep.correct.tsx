import { createMemo, createStore } from "solid-js";
import { trackDeep } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({ count: 0 });
  createMemo(() => trackDeep(store).count);
  return document.createElement("p");
}
