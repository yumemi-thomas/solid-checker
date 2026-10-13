import { onSettled } from "solid-js";
import { createDateNow } from "@solid-primitives/date";
export default function App() {
  onSettled(() => {
    try { createDateNow(0); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
