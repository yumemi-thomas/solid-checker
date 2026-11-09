import { createMemo, createSignal, createTrackedEffect, flush, onCleanup, onSettled } from "solid-js";

// ADR 0918: a leaf callback named by value is walked as the literal is, so an
// operation is reported where it runs in the callback's body.

function wrapUse() {
  createMemo(() => 1);
}

function keep(value: unknown) {
  return value;
}

// Violation at the forEach call: a `const` copy of the callback (cb-alias).
export function AliasForEach() {
  const first = () => {
    const hf = () => {
      createMemo(() => 1);
    };
    [1].forEach(hf);
  };
  const cb = first;
  onSettled(cb);
  return <div />;
}

// The forEach call, as inline: a closed literal's slot (cb-bundle), tracked;
// flush there throws with no diagnostic, so an obligation (ADR 0893).
export function BundleTrackedFlush() {
  const box = {
    cb: () => {
      const hf = () => {
        flush();
      };
      [1].forEach(hf);
    },
  };
  createTrackedEffect(box.cb);
  return <div />;
}

// Violation at the forEach call, once: one body two owners take (cb-shared),
// the operation in a module helper the body's closure calls.
export function SharedWrapper() {
  const cb = () => {
    const hf = () => {
      wrapUse();
    };
    [1].forEach(hf);
  };
  onSettled(cb);
  onSettled(cb);
  return <div />;
}

// Violation at the call, no safe fix: the body is named, so it may have
// other callers the rewrite would change.
export function ConstCleanup() {
  const cb = () => {
    onCleanup(() => {});
  };
  onSettled(cb);
  return <div />;
}

// Violation with the safe fix: the same cleanup written inline (control).
export function LiteralCleanup() {
  onSettled(() => {
    onCleanup(() => {});
  });
  return <div />;
}

// Violation at the call in a function declaration's body.
export function DeclarationPrimitive() {
  function cb() {
    createMemo(() => 1);
  }
  createTrackedEffect(cb);
  return <div />;
}

// Uncertifiable at the member call in the body: a member assignment hands the
// closure on (cb-shared x handoff-assign), as for the inline callback.
export function SharedAssign() {
  const cb = () => {
    const ho: { m?: () => void } = {};
    ho.m = () => {
      createMemo(() => 1);
    };
    ho.m?.();
  };
  onSettled(cb);
  onSettled(cb);
  return <div />;
}

// Uncertifiable at the argument: the slot is written, so the literal does
// not prove what the owner receives (a write to a member of that name
// anywhere vetoes every such slot: its own key here).
export function BundleWritten() {
  const box = {
    step: () => {
      createMemo(() => 1);
    },
  };
  box.step = () => {};
  onSettled(box.step);
  return <div />;
}

// Uncertifiable at the argument: the object escapes to a call.
export function BundleEscaped() {
  const box = {
    cb: () => {
      createMemo(() => 1);
    },
  };
  keep(box);
  onSettled(box.cb);
  return <div />;
}

// Clean: a value-form signal attaches no child.
export function AliasLegal() {
  const first = () => {
    const [count] = createSignal(0);
    count();
  };
  const cb = first;
  onSettled(cb);
  return <div />;
}

// Clean: the closure is created and never run.
export function BundleUnrun() {
  const box = {
    cb: () => {
      const hf = () => {
        createMemo(() => 1);
      };
      void hf;
    },
  };
  createTrackedEffect(box.cb);
  return <div />;
}
