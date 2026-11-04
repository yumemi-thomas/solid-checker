import { action, createMemo, createTrackedEffect, onSettled } from "solid-js";

// An action's first step runs under its callers' leaf owner.
export function FromSettled() {
  const a = createMemo(async () => 1);
  const step = action(function* () {
    a(); // SC5001
  });
  onSettled(() => {
    step();
  });
  return <div />;
}

export function FromTracked() {
  const a = createMemo(async () => 1);
  const step = action(function* () {
    a(); // SC5001
  });
  createTrackedEffect(() => {
    step();
  });
  return <div />;
}

export function PastYield() {
  const a = createMemo(async () => 1);
  const step = action(function* () {
    yield Promise.resolve(1);
    a(); // clean: the step resumed, under no leaf owner
  });
  onSettled(() => {
    step();
  });
  return <div />;
}
