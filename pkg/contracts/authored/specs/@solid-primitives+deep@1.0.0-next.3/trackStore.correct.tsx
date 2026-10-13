import { createMemo, createStore } from "solid-js";
import { trackStore } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({ count: 0 });
  createMemo(() => trackStore(store).count);
  return document.createElement("p");
}
