/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { useKeyDownList } from "@solid-primitives/keyboard";
export default function App() {
  useKeyDownList();
  onSettled(() => {
    try { useKeyDownList(); } catch { /* Retain leaf cleanup diagnostic. */ }
  });
  return <p>ready</p>;
}
