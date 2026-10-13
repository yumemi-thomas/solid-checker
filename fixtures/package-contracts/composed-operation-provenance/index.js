import { createSignal } from "solid-js";

export function readsItsOwnSignal() {
  const [count] = createSignal(1);
  return count();
}

export function readsThroughADirectCall() {
  return readsItsOwnSignal();
}

export function readsThroughAReturnedClosure() {
  return () => readsItsOwnSignal();
}

function privateReader() {
  const [other] = createSignal(2);
  return other();
}

export function readsThroughAPrivateHelper() {
  return privateReader();
}

export function readsUnderTwoNames() {
  const [twice] = createSignal(3);
  return twice();
}

export { readsUnderTwoNames as alsoReadsUnderTwoNames };

export function composesAnAmbiguouslyNamedTarget() {
  return readsUnderTwoNames();
}

export function readsItsOwnSignalAndComposesTheSameShape() {
  const [own] = createSignal(4);
  return own() + readsItsOwnSignal();
}
