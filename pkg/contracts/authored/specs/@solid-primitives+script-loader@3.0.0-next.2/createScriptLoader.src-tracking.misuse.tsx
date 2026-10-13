/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createScriptLoader } from "@solid-primitives/script-loader";
export default function App() {
  const [source, setSource] = createSignal("void 0;");
  createScriptLoader({ src: () => {
    const current = source();
    try { setSource(current); } catch { /* Preserve the write diagnostic while allowing mount. */ }
    return current;
  } });
  return <p>ready</p>;
}
