import { createRoot } from "solid-js";
import { createReducedMotion } from "@solid-primitives/a11y";
createRoot(dispose => {
  createReducedMotion();
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
