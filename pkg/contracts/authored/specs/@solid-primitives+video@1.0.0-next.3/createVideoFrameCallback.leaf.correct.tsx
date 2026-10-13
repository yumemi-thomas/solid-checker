import { onSettled } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  createVideoFrameCallback(() => undefined, () => {});
  return document.createElement("p");
}
