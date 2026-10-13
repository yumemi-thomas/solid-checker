/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createScriptLoader } from "@solid-primitives/script-loader";
export default function App() {
  const [source] = createSignal("void 0;");
  createScriptLoader({ src: () => source() });
  return <p>ready</p>;
}
