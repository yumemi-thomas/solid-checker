/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAutofocus } from "@solid-primitives/focus";
export default function App() {
  onSettled(() => { try { createAutofocus(() => document.body); } catch { /* Keep the emitted dev diagnostic. */ } });
  return <p>ready</p>;
}
