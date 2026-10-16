import { createSignal } from "solid-js";
import { defaultMember, deferredMember, mixedMember, trackedMember, unknownMember } from "./member-helpers";

export function TrackedMember() {
  const [count] = createSignal(0);
  const value = trackedMember({ read: () => count() });
  return <div>{value()}</div>;
}

export function MixedMember() {
  const [count] = createSignal(0);
  const value = mixedMember({ read: () => count() });
  return <div>{value()}</div>;
}

export function DeferredMember() {
  const [count] = createSignal(0);
  deferredMember({ read: () => count() });
  return <div />;
}

export function UnknownMember() {
  const [count] = createSignal(0);
  unknownMember({ read: () => count() });
  return <div />;
}

export function DefaultMember() {
  const [count] = createSignal(0);
  const value = defaultMember({ read: () => count() });
  return <div>{value()}</div>;
}
