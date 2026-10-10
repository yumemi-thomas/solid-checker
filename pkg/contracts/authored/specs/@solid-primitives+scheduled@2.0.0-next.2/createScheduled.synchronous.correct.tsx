import { createMemo } from "solid-js";
import { createScheduled } from "@solid-primitives/scheduled";
export default function App() {
  const scheduled = createScheduled(invalidate => {
    invalidate();
    return () => {};
  });
  createMemo(() => scheduled());
  return document.createElement("p");
}
