// SC7001 missing-effect-function. The dev build's createEffect emits
// MISSING_EFFECT_FN and throws when the apply function is absent. On rc.13 the
// one-argument call is TS2554, so the violation hides the absent apply function
// behind a type assertion, which tsc accepts. The control passes an apply
// function.
import { createEffect, createSignal } from "solid-js";

const [name] = createSignal("ada");

export function Violation() {
  setTimeout(() => {
    createEffect(() => name(), undefined as unknown as (value: string) => void);
  }, 0);
  return <div />;
}

export function Control() {
  createEffect(
    () => name(),
    value => {
      void value;
    }
  );
  return <div />;
}
