import { onSettled } from "solid-js";
import { createPrefersDark } from "@solid-primitives/media";
export default function App() {
  onSettled(() => { try { createPrefersDark(); } catch { /* Keep the package diagnostic and allow mount. */ } });
  return <p>ready</p>;
}
