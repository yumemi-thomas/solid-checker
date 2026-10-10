/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createTimeoutLoop } from "@solid-primitives/timer";
createRoot(dispose => {
  createTimeoutLoop(() => {}, () => false);
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
