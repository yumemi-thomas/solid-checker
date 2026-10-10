import { onSettled } from "solid-js";
import { createDerivedStaticStore } from "@solid-primitives/static-store";
export default function App() {
  onSettled(() => {
    try { createDerivedStaticStore(() => ({ value: 0 })); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
