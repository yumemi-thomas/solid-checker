import { createMemo } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  const [running] = createVideoFrameCallback(() => undefined, () => {});
  const value = running();
  void value;
  return document.createElement("p");
}
