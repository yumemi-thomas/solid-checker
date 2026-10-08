import { createSignal, untrack } from "solid-js";
import { createStaticStore } from "@solid-primitives/static-store";

export default function App() {
  const [count] = createSignal(0);
  untrack(() => createStaticStore({
    get count() { return count(); }
  }));
  return <p>done</p>;
}
