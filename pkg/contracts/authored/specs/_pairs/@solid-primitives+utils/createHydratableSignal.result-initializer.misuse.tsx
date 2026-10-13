import { createSignal } from "solid-js";
import { createHydratableSignal } from "@solid-primitives/utils";
export default function App() {
  const [count, setCount] = createSignal(0);
  createHydratableSignal(() => 0, () => () => { setCount(1); return count(); });
  return <p>{count()}</p>;
}
