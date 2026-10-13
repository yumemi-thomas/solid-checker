import { createSignal } from "solid-js";
import { createLazyMemo } from "@solid-primitives/memo";
export default function App() {
  const [enabled] = createSignal(true);
  const duration = createLazyMemo(() => enabled() ? 300 : 0);
  const initial = duration();
  return <p>{initial}</p>;
}

