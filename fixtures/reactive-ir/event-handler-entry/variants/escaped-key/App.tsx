// ADR 1089 part E round 2: the Fable review's case (coord-land/review-1089e.md, r1089e-key-string2). Chrome rc.21 reports
// PRIMITIVE_IN_EFFECT_CALLBACK at the creation in the handler: the handler-layer proof must not hold here.
import { createEffect, resolve } from "solid-js";

// The stored handler read under an escaped string key into a local, then called.
export function EscapedStringLocal() {
  let el!: HTMLButtonElement;
  createEffect(
    () => 1,
    () => {
      const stored = (el as unknown as Record<string, (() => void) | undefined>)["_\x24\x24click"];
      if (stored) stored();
    },
  );
  return (
    <button
      ref={el}
      onClick={() => {
        resolve(() => 1);
      }}
    >
      go
    </button>
  );
}
