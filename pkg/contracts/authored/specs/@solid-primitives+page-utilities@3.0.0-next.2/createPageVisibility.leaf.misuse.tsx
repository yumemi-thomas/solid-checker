import { onSettled } from "solid-js";
import { createPageVisibility } from "@solid-primitives/page-utilities";
export default function App() {
  onSettled(() => {
    try { createPageVisibility(); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
