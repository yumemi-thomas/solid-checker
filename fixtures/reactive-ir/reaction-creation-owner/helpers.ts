import { createReaction } from "solid-js";

// Created while the module evaluates: no owner is current.
export const trackShared = createReaction(() => {});

// Created by whoever calls the hook, under that caller's owner.
export function useTracker() {
  return createReaction(() => {});
}
