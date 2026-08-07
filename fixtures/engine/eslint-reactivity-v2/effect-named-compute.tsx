import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// A named function passed as an effect's tracked compute. The named-callback
// classifier cannot answer for this position (its tracked arm excludes
// effects), so it must return no role and leave the reads to compiler facts
// rather than fall through to an "untracked rendering" misclassification.
function compute() {
  return count() * 2;
}

export function EffectNamedCompute() {
  createEffect(compute, () => undefined);
  return null;
}
