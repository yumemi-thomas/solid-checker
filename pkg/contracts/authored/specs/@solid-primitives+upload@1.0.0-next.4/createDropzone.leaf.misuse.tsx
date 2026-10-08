import { onSettled } from "solid-js";
import { createDropzone } from "@solid-primitives/upload";
export default function App() {
  onSettled(() => {
    try { createDropzone(); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
