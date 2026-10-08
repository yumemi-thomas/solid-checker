/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createUserTheme } from "@solid-primitives/cookies";
export default function App() {
  onSettled(() => { try { createUserTheme("research-theme"); } catch { /* Preserve the structured leaf diagnostic. */ } });
  return <p>ready</p>;
}
