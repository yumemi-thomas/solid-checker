/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { mapRange } from "@solid-primitives/range";
export default function App() {
  onSettled(() => { try { mapRange(() => 0, () => 3, () => 1, value => value); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
