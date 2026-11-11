import { createMemo, createTrackedEffect, flush, latest, onCleanup, untrack } from "solid-js";

// A callback of a primitive that runs it in place, with a parameter the primitive never passes or async: the slot climb
// does not cross it, and a call written there is an obligation of SC3001 (ADR 0959), never silence.
export function ParamCleanup() {
  createTrackedEffect(() => {
    untrack((p0?: number) => {
      onCleanup(() => {});
    });
  });
  return <p>x</p>;
}
export function AsyncPrimitive() {
  createTrackedEffect(() => {
    void untrack(async () => {
      createMemo(() => 1);
    });
  });
  return <p>x</p>;
}
export function LatestParamFlush() {
  createTrackedEffect(() => {
    latest((p0?: number) => {
      flush();
      return 1;
    });
  });
  return <p>x</p>;
}
// The plain callback: the walk's own proof (a violation).
export function PlainUntrackFlush() {
  createTrackedEffect(() => {
    untrack(() => {
      flush();
    });
  });
  return <p>x</p>;
}
// No call in the callback: nothing to claim.
export function ParamQuiet() {
  createTrackedEffect(() => {
    untrack((p0?: number) => (p0 ?? 1) + 1);
  });
  return <p>x</p>;
}
// Not in a leaf scope: no SC3001 row.
export function InBody() {
  untrack((p0?: number) => {
    onCleanup(() => {});
  });
  return <p>x</p>;
}
