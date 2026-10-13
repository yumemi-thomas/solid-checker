import { createMemo, createSignal } from "solid-js";
import { createUndoHistory } from "@solid-primitives/history";
export default function App() {
  const [count, setCount] = createSignal(0);
  const history = createUndoHistory(() => { const current = count(); return () => setCount(current); });
  const current = history.canUndo();
  void current;
  return document.createElement("p");
}
