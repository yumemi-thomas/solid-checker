// app-game learn-solidjs-utils/transient-component-demo.tsx:206-214: a prop is read in the
// callbacks of a promise chain started in a tracked effect.
import { createTrackedEffect } from "solid-js";

export function FadeOut(props: { onFinish?: () => void }) {
  createTrackedEffect(() => {
    void fetch("/ping").then(() => {
      props.onFinish?.();
    });
  });
  return <p>Fading</p>;
}
