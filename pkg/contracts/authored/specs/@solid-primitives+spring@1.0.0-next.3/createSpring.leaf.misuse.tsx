/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  onSettled(() => { try { createSpring(0); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
