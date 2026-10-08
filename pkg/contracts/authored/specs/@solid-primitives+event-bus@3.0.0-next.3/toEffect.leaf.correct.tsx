import { onSettled } from "solid-js";
import { toEffect } from "@solid-primitives/event-bus";
export default function App() {
  toEffect<string>(() => {});
  onSettled(() => {});
  return <p>ready</p>;
}
