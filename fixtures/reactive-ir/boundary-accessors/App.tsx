import { createErrorBoundary, createLoadingBoundary } from "solid-js/internal";

export function Reads() {
  const caught = createErrorBoundary(() => 1, () => 2);
  const held = createLoadingBoundary(() => 1, () => 2);
  caught();
  held();
  return <p />;
}

export function Tracked() {
  const caught = createErrorBoundary(() => 1, () => 2);
  const held = createLoadingBoundary(() => 1, () => 2);
  return <p>{[caught(), held()]}</p>;
}
