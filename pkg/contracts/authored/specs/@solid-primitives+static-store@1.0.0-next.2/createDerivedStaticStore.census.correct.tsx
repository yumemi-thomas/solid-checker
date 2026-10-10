import { createSignal } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";

export default function App() {
  const [count] = createSignal(0);
  createDerivedStaticStore(() => {
    count();
    return { value: count() };
  });
  return <p>{count()}</p>;
}
