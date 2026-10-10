import { onSettled } from "solid-js";
import { createMicrotask } from "@solid-primitives/utils";
export default function App() {
  onSettled(() => { try { createMicrotask(() => {}); } catch { /* Keep the package diagnostic and allow mount. */ } });
  return <p>ready</p>;
}
