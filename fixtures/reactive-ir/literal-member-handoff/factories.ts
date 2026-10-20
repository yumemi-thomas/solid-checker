import { createSignal } from "solid-js";

// Replaces the member through a generic parameter: the write names the
// constraint's `label`, never the caller literal's property symbol.
export function relabel<T extends { label: () => string }>(target: T): T {
  target.label = () => "fixed";
  return target;
}

// Closed: the literal's binding is used only for its return.
export function makeKept() {
  const [label] = createSignal("kept");
  const tag = { label };
  return tag;
}

// Closed: an expression-bodied factory returning the literal itself.
export const makeArrow = () => {
  const [label] = createSignal("arrow");
  return { label };
};

// Factory-internal hand-off: the literal reaches relabel before it is
// returned.
export function makeHanded() {
  const [label] = createSignal("handed");
  const tag = { label };
  relabel(tag);
  return tag;
}

// Closed here; its one consumer hands the object off (consumer side).
export function makePlain() {
  const [label] = createSignal("plain");
  const tag = { label };
  return tag;
}
