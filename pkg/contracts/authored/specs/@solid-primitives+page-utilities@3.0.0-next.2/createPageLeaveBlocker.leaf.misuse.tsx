import { onSettled } from "solid-js";
import { createPageLeaveBlocker } from "@solid-primitives/page-utilities";
export default function App() {
  onSettled(() => {
    try { createPageLeaveBlocker(true); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
