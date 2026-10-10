import { createSignal } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";

export default function App() {
  const [count, setCount] = createSignal(0);
  createDerivedStaticStore(() => {
    setCount(1);
    return { value: count() };
  });
  return <p>{count()}</p>;
}
