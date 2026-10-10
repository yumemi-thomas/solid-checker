import { onCleanup, onSettled } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  onSettled(() => { createVideoFrameCallback(() => { onCleanup(() => {}); return undefined; }, () => {}); });
  return document.createElement("p");
}
