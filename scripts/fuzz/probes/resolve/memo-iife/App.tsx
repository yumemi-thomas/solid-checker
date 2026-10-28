// ADR 0494 (from the misuse matrix): resolve() in an IIFE inside a memo compute runs with the observer active (RESOLVE_IN_REACTIVE_SCOPE).
import { createMemo, createSignal, resolve as resolveAlias } from "solid-js";

export function C1() {
  const m1 = createMemo(() => {
    (() => {
      resolveAlias(() => 1);
    })();
    return 1;
  });
  return (
    <div>
      <p data-view="v1">{m1()}</p>
    </div>
  );
}
