// app-game gpu-text-rendering FrameLoop.tsx:41-42, createFrameScheduler.ts:38: a createEffect apply argument that is a
// method of a project factory's result.
import { createEffect, createSignal } from "solid-js";

function createScheduler() {
  let active = false;
  return {
    setActive(value: boolean) {
      active = value;
    },
  };
}

export function FrameLoop() {
  const [visible] = createSignal(true);
  const loop = createScheduler();
  createEffect(visible, loop.setActive);
  return <canvas />;
}
