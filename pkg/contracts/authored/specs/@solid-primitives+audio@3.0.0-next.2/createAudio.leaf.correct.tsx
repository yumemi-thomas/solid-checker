/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  createAudio("");
  onSettled(() => {});
  return <p>ready</p>;
}
