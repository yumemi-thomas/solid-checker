import { createEffect, createRoot, createSignal, flush } from "solid-js";
import { effectOfThisCopy } from "./barrel";
import * as Bridge from "./lib/bridge";
import { createEffect as effectOfOtherCopy } from "./lib/bridge";
import { createEffect as effectThroughStar } from "./lib/star";
import { createEffect as effectThroughPackage } from "foreign-effects";

createRoot(() => {
  const [value] = createSignal(1);

  // Positive: `createEffect` and `flush` load the same copy.
  createEffect(value, () => {
    flush();
  });

  // Positive: a barrel in this directory re-exports the same copy.
  effectOfThisCopy(value, () => {
    flush();
  });

  // Negative: `lib/bridge.ts` re-exports the second copy, which has a queue of
  // its own, so this `flush` drains the other one.
  effectOfOtherCopy(value, () => {
    flush();
  });

  // Negative: the same through `export *`, and through a namespace import.
  effectThroughStar(value, () => {
    flush();
  });
  Bridge.createEffect(value, () => {
    flush();
  });

  // Negative: a package's `exports` conditions pick the file it loads, and with
  // it which installed `solid-js` that file imports (here the same copy, but
  // the conditions are not the ones this proof selects): not claimed.
  effectThroughPackage(value, () => {
    flush();
  });
});
