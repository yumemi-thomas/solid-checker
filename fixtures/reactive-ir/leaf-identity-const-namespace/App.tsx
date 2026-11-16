import { createTrackedEffect } from "solid-js";
import * as S from "solid-js";

// A `const` over the namespace import: the slot names the primitive by a plain key and is never rebound.
const { createEffect: makeEffect } = S;
const { createMemo } = S;
const { onCleanup: cleanupLater, ...others } = S;
export const { flush: flushNow } = S;
const Held = S;
const Again = Held;
const { createEffect: makeHeld } = Again;

// No exact identity: a `let`, a default, a nested pattern, a computed key, a `let` holder.
let { createEffect: makeLet } = S;
const { createEffect: makeDefault = S.createEffect } = S;
const { inner: { createEffect: makeNested } } = { inner: S };
const { ["createEffect"]: makeComputed } = S;
let Loose = S;

// Not the namespace: a project object with the primitive's name.
const { createEffect: mine } = { createEffect: (compute: () => number, apply: () => void) => { void compute; void apply; } };

// Violations: SC3001, a creation, a cleanup and a flush in a leaf owner, each named through the `const`.
export function Destructured() {
  createTrackedEffect(() => {
    makeEffect(() => 1, () => {});
  });
  return <div></div>;
}

export function Shorthand() {
  createTrackedEffect(() => {
    createMemo(() => 1);
  });
  return <div></div>;
}

export function WithRest() {
  void others;
  createTrackedEffect(() => {
    cleanupLater(() => {});
  });
  return <div></div>;
}

export function ExportedSlot() {
  createTrackedEffect(() => {
    flushNow();
  });
  return <div></div>;
}

export function HeldDestructured() {
  createTrackedEffect(() => {
    makeHeld(() => 1, () => {});
  });
  return <div></div>;
}

// Obligations: `leaf-operation-identity-unproven`, and a member of a held namespace or of a project object, which no primitive is
// attributed to (SC9012 `leaf-callback-opaque`).
export function HeldNamespace() {
  createTrackedEffect(() => {
    Again.createEffect(() => 1, () => {});
  });
  return <div></div>;
}

export function LetSlot() {
  createTrackedEffect(() => {
    makeLet(() => 1, () => {});
  });
  return <div></div>;
}

export function DefaultSlot() {
  createTrackedEffect(() => {
    makeDefault(() => 1, () => {});
  });
  return <div></div>;
}

export function NestedSlot() {
  createTrackedEffect(() => {
    makeNested(() => 1, () => {});
  });
  return <div></div>;
}

export function ComputedSlot() {
  createTrackedEffect(() => {
    makeComputed(() => 1, () => {});
  });
  return <div></div>;
}

export function LetHolder() {
  createTrackedEffect(() => {
    Loose.createEffect(() => 1, () => {});
  });
  return <div></div>;
}

export function OwnObject() {
  createTrackedEffect(() => {
    mine(() => 1, () => {});
  });
  return <div></div>;
}

// Silent: the `const` slot called where an owner is current.
export function OwnedBody() {
  makeEffect(() => 1, () => {});
  return <div></div>;
}
