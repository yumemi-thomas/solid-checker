import { createMemo, createStore } from "solid-js";
import { trackStore } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({});
  createMemo(() => trackStore(store));
  return document.createElement("p");
}
