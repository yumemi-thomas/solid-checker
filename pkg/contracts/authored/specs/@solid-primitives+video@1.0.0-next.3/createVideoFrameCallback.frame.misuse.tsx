import { getOwner, onCleanup } from "solid-js";
import { createVideoFrameCallback } from "@solid-primitives/video";
export default function App() {
  const owner = getOwner();
  void owner;
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  const video = document.createElement("video");
  video.muted = true;
  video.srcObject = canvas.captureStream(10);
  document.body.append(video);
  const paint = setInterval(() => context?.fillRect(0, 0, 20, 20), 20);
  onCleanup(() => { clearInterval(paint); video.pause(); });
  const done = document.createElement("p");
  done.id = "done";
  const [, start, stop] = createVideoFrameCallback(() => video, () => {
    onCleanup(() => {});
    done.textContent = "done";
    stop();
  });
  const button = document.createElement("button");
  button.id = "target";
  button.onclick = () => { void video.play().then(() => start()); };
  document.body.append(button);
  return done;
}
