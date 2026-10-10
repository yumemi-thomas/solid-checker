/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createSelection } from "@solid-primitives/selection";
export default function App() {
  onSettled(() => { try { createSelection(); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
