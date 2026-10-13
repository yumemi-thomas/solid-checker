import { createMemo } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  const [running] = createVideoFrameCallback(() => undefined, () => {});
  createMemo(() => running());
  return document.createElement("p");
}
