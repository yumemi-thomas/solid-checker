import {
  type Component,
  createMemo,
  createSignal,
  onCleanup,
  requestCallback
} from "solid-js";
import { createDebounce } from "@solid-primitives/debounce";

// requestCallback runs later, outside reactive tracking. The bundled Solid
// 1.x row certifies that timing instead of treating the callback as a negative
// (never invoked) claim.
export const Scheduled: Component = () => {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  requestCallback(() => doubled());
  return <div>{count()}</div>;
};

// createDebounce itself registers cleanup in the component's owner. Its user
// callback runs later from a timer without an owner or tracking context, so
// both the owner requirement and deferred callback are contract-driven.
export const Debounced: Component = () => {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  const run = createDebounce(() => doubled(), 0);
  onCleanup(run.clear);
  return <button onClick={run}>Run</button>;
};
