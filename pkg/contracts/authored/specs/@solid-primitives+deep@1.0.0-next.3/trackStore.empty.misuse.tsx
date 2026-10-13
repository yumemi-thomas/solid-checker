import { createStore } from "solid-js";
import { trackStore } from "@solid-primitives/deep";
export default function App() {
  const [store] = createStore({});
  trackStore(store);
  return document.createElement("p");
}
