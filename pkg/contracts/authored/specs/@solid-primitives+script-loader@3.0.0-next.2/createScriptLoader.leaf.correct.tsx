/** @jsxImportSource @solidjs/web */
import { createScriptLoader } from "@solid-primitives/script-loader";
export default function App() {
  createScriptLoader({ src: "void 0;" });
  return <p>ready</p>;
}
