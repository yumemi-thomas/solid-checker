import { createEffect, createMemo, untrack } from "solid-js";
import { render } from "@solidjs/web";

// ADR 1213, the effect side. A callback a reviewed scheduler runs on a fresh stack is no part of the apply that schedules it: `ApplyTimer` is silent.
// Layers that run in place keep the apply's stack: `ApplyUntrack` and `ApplyNamed` throw PRIMITIVE_IN_EFFECT_CALLBACK, and so do a module function
// that shadows `setTimeout` and an object's `setTimeout` member, each of which runs its argument at once.
function setTimeout(fn: () => void, _ms?: number): void {
  fn();
}
const sched = { setTimeout: (fn: () => void) => fn() };

function ApplyTimer() {
  createEffect(
    () => 0,
    () => {
      globalThis.setTimeout(() => {
        document.body.dataset.timer = "ran";
        createMemo(() => 1);
      }, 0);
    },
  );
  return <p>t</p>;
}
function ApplyUntrack() {
  createEffect(
    () => 0,
    () => {
      untrack(() => {
        createMemo(() => 2);
      });
    },
  );
  return <p>u</p>;
}
function ApplyNamed() {
  createEffect(
    () => 0,
    () => {
      const make = () => createMemo(() => 3);
      make();
    },
  );
  return <p>n</p>;
}
function ApplyShadowed() {
  createEffect(
    () => 0,
    () => {
      setTimeout(() => {
        createMemo(() => 4);
      }, 0);
    },
  );
  return <p>s</p>;
}
function ApplyMember() {
  createEffect(
    () => 0,
    () => {
      sched.setTimeout(() => {
        createMemo(() => 5);
      });
    },
  );
  return <p>m</p>;
}

render(
  () => (
    <p>
      <ApplyTimer />
      <ApplyUntrack />
      <ApplyNamed />
      <ApplyShadowed />
      <ApplyMember />
    </p>
  ),
  document.body,
);
