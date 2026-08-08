// Two bindings spelled `apply`, only one of which is a callback.
//
// `Host`'s options object names its own destructured parameter. The
// module-scoped `apply` below shares nothing with it but the spelling: a
// different scope, a different declaration, a different TypeScript symbol.
// Matching a callback position to a function by comparing source text admits
// it anyway, and then classifies its read of `count` as running in the
// effect-apply phase of an effect it is not a callback of.
import { createEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

function apply(value: number) {
  setCount(count() + value);
}

apply(1);

export function Host(options: { apply: (value: number) => void }) {
  const { apply } = options;
  createEffect(() => count(), { effect: apply });
}
