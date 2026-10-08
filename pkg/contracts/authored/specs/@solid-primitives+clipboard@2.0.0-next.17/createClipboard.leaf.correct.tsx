import { onSettled } from "solid-js";
import { createClipboard } from "@solid-primitives/clipboard";
export default function App() {
  createClipboard();
  onSettled(() => {});
  return <p>ready</p>;
}
