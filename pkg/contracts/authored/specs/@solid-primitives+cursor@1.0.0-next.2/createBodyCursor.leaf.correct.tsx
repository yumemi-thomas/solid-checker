import { onSettled } from "solid-js";
import { createBodyCursor } from "@solid-primitives/cursor";
export default function App() {
  createBodyCursor(() => "pointer");
  onSettled(() => {});
  return <p>ready</p>;
}
