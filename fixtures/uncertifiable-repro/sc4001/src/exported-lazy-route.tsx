// app-game night-defense StartMenuScene.tsx:89 with routes.tsx:8: a default
// export loaded by `lazy(() => import(...))` creates an effect in its body.
import { createSignal, createTrackedEffect } from "solid-js";

export default function StartMenuScene() {
  const [size, setSize] = createSignal({ width: 800, height: 600 });
  const scene = <canvas width={size().width} height={size().height} onClick={() => setSize({ width: 400, height: 300 })} />;
  createTrackedEffect(() => console.debug("layout", Math.max(1, size().width)));
  return scene;
}
