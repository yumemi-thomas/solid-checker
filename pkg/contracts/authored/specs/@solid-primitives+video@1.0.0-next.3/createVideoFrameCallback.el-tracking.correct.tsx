import { createSignal } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  const [value, setValue] = createSignal(0);
  let calls = 0;
  createVideoFrameCallback(() => {
    calls++;
    const current = value();
    void current;
    return undefined;
  }, () => {});
  return document.createElement("p");
}
