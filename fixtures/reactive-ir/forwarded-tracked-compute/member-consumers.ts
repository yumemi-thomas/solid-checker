import { createMemo, untrack } from "solid-js";
type Consumer = { compute: () => number };
declare function keep(value: unknown): void;

export function trackedSink(options: Consumer) {
  return createMemo(() => options.compute());
}
export function mixedSink(options: Consumer) {
  options.compute();
  return createMemo(() => options.compute());
}
export function freshSink(options: Consumer) {
  setTimeout(() => { options.compute(); }, 0);
}
export function sampledSink(options: Consumer) {
  return untrack(() => options.compute());
}
export function escapedSink(options: Consumer) {
  keep(options);
}

export function siblingSink(options: Consumer & { sibling: () => number }) {
  options.sibling();
  return createMemo(() => options.compute());
}
