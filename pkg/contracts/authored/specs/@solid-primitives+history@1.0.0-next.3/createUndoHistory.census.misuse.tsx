import { createSignal } from "solid-js";
import { createUndoHistory } from "@solid-primitives/history";

export default function App() {
  const [count, setCount] = createSignal(0); const history = createUndoHistory(() => { const value = count(); return () => setCount(value); });
  const current = history.canUndo();
  return <p>{String(current)}</p>;
}
