/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
createRoot(dispose => {
  createFullscreen(() => document.body);
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
