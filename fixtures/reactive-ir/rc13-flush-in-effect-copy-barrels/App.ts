import { createEffect, createRoot, createSignal, flush } from "solid-js";
import { effectOfThisCopy } from "./barrel";
import * as Bridge from "./lib/bridge";
import { createEffect as effectOfOtherCopy } from "./lib/bridge";
import { createEffect as effectThroughStar } from "./lib/star";

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
});
