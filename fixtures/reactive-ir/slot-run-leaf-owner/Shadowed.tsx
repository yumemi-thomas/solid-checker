import { createTrackedEffect, onCleanup } from "solid-js";

// A function named like a primitive is not the primitive: this one keeps its callback, so nothing here runs it
// inside the leaf scope (and nothing crosses it by name).
const kept: Array<() => void> = [];
const untrack = (fn: () => void) => {
  kept.push(fn);
};

export function ShadowedUntrack() {
  createTrackedEffect(() => {
    untrack(() => {
      onCleanup(() => {});
    });
  });
  return <div />;
}
