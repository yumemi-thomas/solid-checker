import { createSignal } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";
export default function App() {
  const [count] = createSignal(0);
  createDerivedStaticStore(() => ({ get value() { return count(); } }));
  return <p>derived</p>;
}
