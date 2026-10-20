import { createSignal } from "solid-js";
import { wrapped, wrappedAndEager, wrappedLater, wrappedSnapshot, wrappedUnknown } from "./wrappers";

// Source-visible typed adapter: read only under the inner memo's observer.
export function UsesWrappedOptions() {
  const [open] = createSignal(false);
  const options = wrapped(() => ({ enabled: open() }));
  return <main>{options().enabled}</main>;
}

// The same signature cannot hide an additional untracked invocation.
export function UsesWrappedAndEager() {
  const [open] = createSignal(false);
  const options = wrappedAndEager(() => ({ enabled: open() }));
  return <main>{options().enabled}</main>;
}

// Explicit snapshot: untrack opts out of strict-read reporting.
export function UsesWrappedSnapshot() {
  const [open] = createSignal(false);
  const options = wrappedSnapshot(() => ({ enabled: open() }));
  return <main>{options().enabled}</main>;
}

// Returning the invocation does not prove when or how that callback runs.
export function UsesWrappedLater() {
  const [open] = createSignal(false);
  const options = wrappedLater(() => ({ enabled: open() }));
  return <main>{options().enabled}</main>;
}

// A declared signature supplies no execution or tracking evidence.
export function UsesWrappedUnknown() {
  const [open] = createSignal(false);
  const options = wrappedUnknown(() => ({ enabled: open() }));
  return <main>{options().enabled}</main>;
}
