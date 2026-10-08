/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAutofocus } from "@solid-primitives/focus";
export default function App() {
  createAutofocus(() => document.body);
  onSettled(() => {});
  return <p>ready</p>;
}
