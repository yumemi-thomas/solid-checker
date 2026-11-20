import { action, createEffect, createStore } from "solid-js";

// Uncertifiable: an effect's compute named by its binding and also called in an action step the apply runs.
export function ActionArrow() {
  const [s1] = createStore({ v: 0 });
  const a1 = action(function* () {
    const f = () => s1.v;
    createEffect(f, () => {});
    f();
  });
  createEffect(() => 1, () => {
    a1();
  });
  return <div />;
}

// Uncertifiable: the same through a named function expression (ADR 1127).
export function ActionNamed() {
  const [s1] = createStore({ v: 0 });
  const a1 = action(function* () {
    const f = function inner() { return s1.v; };
    createEffect(f, () => {});
    f();
  });
  createEffect(() => 1, () => {
    a1();
  });
  return <div />;
}

// Clean: the compute is the function's only entry.
export function ActionComputeOnly() {
  const [s1] = createStore({ v: 0 });
  const a1 = action(function* () {
    const f = () => s1.v;
    createEffect(f, () => {});
  });
  createEffect(() => 1, () => {
    a1();
  });
  return <div />;
}

// Violation at the call: in the body.
export function BodyCalled() {
  const [s1] = createStore({ v: 0 });
  const f = () => s1.v;
  createEffect(f, () => {});
  f();
  return <div />;
}
