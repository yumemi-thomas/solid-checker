import { lazy } from "solid-js";
import { createLoop } from "dep-loop";

const frame = () => {};
const flag = Math.random() > 0.5;

// A conditional `default` value. Not a lazy-default witness (the value is no
// plain identifier), and the object-retention guard compares only an exact
// property value, so it never refused this read: `start()` was a violation
// before ADR 0292 and stays one. Pinned, not claimed by the new proof.
const ConditionalLoop = lazy(async () => {
  function ConditionalCube() {
    const [running, start] = createLoop(frame);
    start();
    return <div>{String(running())}</div>;
  }
  function Plain() {
    return <div />;
  }
  return { default: flag ? ConditionalCube : Plain };
});

export function ConditionalPage() {
  return (
    <main>
      <ConditionalLoop />
    </main>
  );
}
