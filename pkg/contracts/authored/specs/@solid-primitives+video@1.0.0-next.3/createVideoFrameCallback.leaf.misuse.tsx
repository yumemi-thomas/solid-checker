import { onSettled } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  onSettled(() => { createVideoFrameCallback(() => undefined, () => {}); });
  return document.createElement("p");
}
