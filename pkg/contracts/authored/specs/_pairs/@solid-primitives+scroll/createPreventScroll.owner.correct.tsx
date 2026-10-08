/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createPreventScroll } from "@solid-primitives/scroll";
createRoot(dispose => {
  createPreventScroll({ enabled: false });
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
