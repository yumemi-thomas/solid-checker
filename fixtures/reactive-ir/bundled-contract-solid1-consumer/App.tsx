import {
  type Component,
  createMemo,
  createSignal,
  onCleanup,
  requestCallback
} from "solid-js";
import { createDebounce } from "@solid-primitives/debounce";

// The fixture's minimal solid-js manifest selects the v1 dialect, but it does
// not contain the exact published runtime artifact covered by the first-party
// receipt. Native v1 timing still proves this callback runs later untracked.
export const Scheduled: Component = () => {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  requestCallback(() => doubled());
  return <div>{count()}</div>;
};

// This reduced debounce implementation also differs from the published
// artifact. Its local body proves the later untracked callback without
// borrowing the published package's accepted contract by name.
export const Debounced: Component = () => {
  const [count] = createSignal(0);
  const doubled = createMemo(() => count() * 2);
  const run = createDebounce(() => doubled(), 0);
  onCleanup(run.clear);
  return <button onClick={run}>Run</button>;
};
