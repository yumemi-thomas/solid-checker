import { createRoot } from "solid-js";
import { createClipboard } from "@solid-primitives/clipboard";
createRoot(dispose => {
  createClipboard();
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
