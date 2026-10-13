import { onSettled } from "solid-js";
import { createElementCursor } from "@solid-primitives/cursor";
export default function App() {
  createElementCursor(document.body, "pointer");
  onSettled(() => {});
  return <p>ready</p>;
}
