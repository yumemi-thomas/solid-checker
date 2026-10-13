/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createPageLeaveBlocker } from "@solid-primitives/page-utilities";
createRoot(dispose => {
  createPageLeaveBlocker();
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
