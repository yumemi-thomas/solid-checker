/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createScriptLoader } from "@solid-primitives/script-loader";
export default function App() {
  onSettled(() => { try { createScriptLoader({ src: "void 0;" }); } catch { /* Preserve the structured leaf diagnostic. */ } });
  return <p>ready</p>;
}
