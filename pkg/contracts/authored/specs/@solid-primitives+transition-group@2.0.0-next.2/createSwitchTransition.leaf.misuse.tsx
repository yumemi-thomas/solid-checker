/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createSwitchTransition } from "@solid-primitives/transition-group";
export default function App() {
  onSettled(() => { try { createSwitchTransition(() => 1, {}); } catch { /* Keep the emitted dev diagnostic. */ } });
  return <p>ready</p>;
}
