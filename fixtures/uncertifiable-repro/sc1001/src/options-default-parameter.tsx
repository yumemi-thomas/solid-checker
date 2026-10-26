// app-game gpu-text-rendering CameraTour.tsx:21 (useFrame, features/scene/FrameLoop.tsx:81): callbacks reading props sit in an options
// object handed to a project function whose options parameter has a default value (`options = {}`), which its parameter proof does not follow.
import { createEffect, createSignal } from "solid-js";

function useFrame(callback: () => void, options: { enabled?: () => boolean; onTick?: () => void } = {}) {
  createEffect(
    () => options.enabled?.() ?? true,
    (enabled) => {
      if (enabled) callback();
      options.onTick?.();
    },
  );
}

export function Tour(props: { enabled: boolean }) {
  const [ticks, setTicks] = createSignal(0);
  useFrame(() => {}, { enabled: () => props.enabled, onTick: () => setTicks((count) => count + 1) });
  return <p>{ticks()}</p>;
}
