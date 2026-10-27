// probus-hk lib/tanstack/table.tsx:115: a Solid primitive (`untrack`) is stored as an
// object member value instead of being called.
import { untrack } from "solid-js";

export const reactivity = {
  untrack,
  batch: (fn: () => void) => fn(),
};
