import { createSignal } from "solid-js";
import { createAggregated } from "@solid-primitives/async";
export default function App() {
  const [count, setCount] = createSignal(0);
  createAggregated(() => ({ get value() { setCount(1); return count(); } }));
  return <p>{count()}</p>;
}
