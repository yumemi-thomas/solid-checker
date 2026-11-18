import { createEffect, createMemo, createRenderEffect, createSignal } from "solid-js";

// ADR 1111: a read written after a creation the effect-callback guard refuses or may refuse (SC6002, violation or
// obligation), in the same function or in a function nested in it, may never run: PRIMITIVE_IN_EFFECT_CALLBACK throws first.
// SC1001 is uncertifiable `creation-throws-first` there.

// A helper an effect's apply calls: `createMemo(f)` throws before `f()` reads.
export function ReadAfterCreationInHelper() {
  const [count] = createSignal(0);
  const use = () => {
    const f = () => count();
    createMemo(f);
    f();
  };
  createEffect(
    () => 1,
    () => {
      use();
    },
  );
  return <i />;
}
// Directly in a render effect's apply that the owned first run leaves before the creation: every run that reaches the
// read is a rerun, where the guard throws first (SC6002 an obligation).
export function ReadAfterGuardedRenderCreation() {
  const [count] = createSignal(0);
  const [step, setStep] = createSignal(1);
  createRenderEffect(
    () => step(),
    (v) => {
      if (v === 1) return;
      createMemo(() => 1);
      count();
    },
  );
  return <button onClick={() => setStep(2)} />;
}
// Kept: the read is written before the creation in the same function, so it runs first.
export function ReadBeforeCreation() {
  const [count] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      count();
      createMemo(() => 1);
    },
  );
  return <i />;
}
// Kept: no creation in the apply.
export function ReadInApply() {
  const [count] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      count();
    },
  );
  return <i />;
}
