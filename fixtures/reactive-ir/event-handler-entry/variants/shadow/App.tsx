// ADR 1089 part E round 2: the Fable review's case (coord-land/review-1089e.md, r1089e-shadow-min). Chrome rc.21 reports
// PRIMITIVE_IN_EFFECT_CALLBACK at the creation in the handler: the handler-layer proof must not hold here.
import { createEffect, createMemo } from "solid-js";

// Minimal: no dispatch call, no key read, nothing the part E scan refuses.
export function ShadowIframeLoad() {
  let host!: HTMLDivElement;
  const frame = (
    <iframe
      onLoad={() => {
        createMemo(() => 1);
      }}
    />
  ) as HTMLIFrameElement;
  createEffect(
    () => 1,
    () => {
      queueMicrotask(() => 0);
      host.attachShadow({ mode: "open" }).appendChild(frame);
    },
  );
  return <div ref={host} />;
}
