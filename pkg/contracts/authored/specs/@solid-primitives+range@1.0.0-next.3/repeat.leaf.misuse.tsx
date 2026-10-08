/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { repeat } from "@solid-primitives/range";
export default function App() {
  onSettled(() => { try { repeat(() => 3, index => index); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
