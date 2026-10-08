/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  onSettled(() => { try { createAudio(""); } catch { /* Keep the dev diagnostic. */ } });
  return <p>ready</p>;
}
