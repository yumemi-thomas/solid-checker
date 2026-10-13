import { onSettled } from "solid-js";
import { createPointerListeners } from "@solid-primitives/pointer";
export default function App() {
  onSettled(() => { try { createPointerListeners({ target: document.body, onDown: () => {} }); } catch { /* Keep the package diagnostic and allow mount. */ } });
  return <p>ready</p>;
}
