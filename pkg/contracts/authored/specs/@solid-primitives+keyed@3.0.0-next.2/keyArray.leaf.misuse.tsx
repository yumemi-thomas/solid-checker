/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { keyArray } from "@solid-primitives/keyed";
export default function App() {
  onSettled(() => { try { keyArray(() => [1, 2], item => item, value => value()); } catch { /* Preserve the structured leaf diagnostic. */ } });
  return <p>ready</p>;
}
