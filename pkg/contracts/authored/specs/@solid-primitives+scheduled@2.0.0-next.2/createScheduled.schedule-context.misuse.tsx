import { createMemo, createSignal } from "solid-js";
import { createScheduled } from "@solid-primitives/scheduled";
export default function App() {
  const [value, setValue] = createSignal(0);
  createMemo(() => createScheduled(() => {
    const current = value();
    setValue(current + 1);
    return () => {};
  }));
  return document.createElement("p");
}
