import { createTrackedEffect } from "solid-js";
import * as S from "solid-js";
import * as M from "./mine";

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

// The review of ADR 1007. Obligations or no row: a block-scoped object that shadows the import, a namespace of a project
// module, a holder of that one, and a `const` written in a `switch` case clause that a later clause uses (a jump to the later
// clause skips the declarator).
export function ShadowedNamespace() {
  const S = { createEffect: (compute: () => number, apply: () => void) => { void compute; void apply; } };
  const { createEffect: makeShadow } = S;
  createTrackedEffect(() => {
    makeShadow(() => 1, () => {});
  });
  return <div></div>;
}

export function ProjectNamespace() {
  const { createEffect: makeMine } = M;
  createTrackedEffect(() => {
    makeMine(() => 1, () => {});
  });
  return <div></div>;
}

export function ProjectNamespaceHeld() {
  const HeldMine = M;
  const { createEffect: makeHeldMine } = HeldMine;
  createTrackedEffect(() => {
    makeHeldMine(() => 1, () => {});
  });
  return <div></div>;
}

export function SwitchCaseSlot(props: { step: number }) {
  switch (props.step) {
    case 0:
      const { createEffect: makeLate } = S;
      void makeLate;
      break;
    case 1:
      createTrackedEffect(() => {
        makeLate(() => 1, () => {});
      });
  }
  return <div></div>;
}
