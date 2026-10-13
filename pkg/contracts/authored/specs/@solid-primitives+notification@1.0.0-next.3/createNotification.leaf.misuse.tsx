/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createNotification } from "@solid-primitives/notification";
export default function App() {
  onSettled(() => { try { createNotification("hello"); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
