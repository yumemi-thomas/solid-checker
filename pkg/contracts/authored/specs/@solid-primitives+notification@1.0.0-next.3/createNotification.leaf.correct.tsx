/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createNotification } from "@solid-primitives/notification";
export default function App() {
  createNotification("hello");
  onSettled(() => {});
  return <p>ready</p>;
}
