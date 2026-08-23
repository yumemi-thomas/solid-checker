import { createSignal } from "solid-js";
import { observe } from "uncharted-helpers";

export function Derived() {
  const [value] = createSignal(0);
  const derived = observe(value);
  return { derived };
}

export function Held() {
  const [value] = createSignal(0);
  observe(value);
  return { value };
}

export function Steady() {
  return { view: 1 };
}
