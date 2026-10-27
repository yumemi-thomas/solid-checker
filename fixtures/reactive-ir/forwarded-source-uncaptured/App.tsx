import { createMemo, createSignal } from "solid-js";
import { observe } from "dep-ext";
import { relay } from "./relay";

interface Api {
  get(): (s: () => number) => void;
}
declare const api: Api;

// Each helper hands its parameter on to a callee with no analyzed body:
// `observe(count)` or `fn(count)` written here is an SC9012 `package-callee`
// obligation; the call through the helper is an SC9012 obligation too
// (ADR 0370, 0440).
function watch(source: () => number): void {
  observe(source);
}

function viaValue(source: () => number): void {
  const fn = api.get();
  fn(source);
}

// Negative: an exact project callee that passes the parameter to a tracked
// primitive.
function tracked(source: () => number): () => number {
  return createMemo(source);
}

function forward(source: () => number): () => number {
  return tracked(source);
}

export function Panel() {
  const [count] = createSignal(1);
  watch(count);
  viaValue(count);
  relay(count);
  const same = forward(count);
  return <p>{same()}</p>;
}
