import { createSignal } from "solid-js";
import { observe } from "dep-ext";

type S = () => number;
// Negative: a spread adds zero or more arguments, so `count` lands at
// position 1 or later, the rest parameter, which reaches nothing.
function firstOnly(first: S, ...rest: S[]) {
  observe(first);
}
// Transports a bare parameter reference does not cover: each is an SC9012
// obligation at `count` (ADR 0370).
function restElement(...sources: S[]) {
  observe(sources[0]);
}
function argumentsObject(_s: S) {
  observe(arguments[0]);
}
function restSpread(...args: S[]) {
  observe(...(args as [S]));
}
function direct(s: S) {
  observe(s);
}

export function Panel() {
  const [count] = createSignal(1);
  firstOnly(() => 0, ...([] as []), count);
  firstOnly(...([] as []), count);
  restElement(count);
  argumentsObject(count);
  restSpread(count);
  direct.call(null, count);
  direct.apply(null, [count]);
  return <p>{count()}</p>;
}
