import { createEffect, createRoot, getOwner, onCleanup, runWithOwner } from "solid-js";
import { runNow } from "./helpers";

function call(cb: () => unknown) {
  cb();
  return 1;
}

let kept: (() => unknown) | undefined;
function callAndKeep(cb: () => unknown) {
  cb();
  kept = cb;
}

function keep(cb: () => unknown) {
  kept = cb;
}

const on = Date.now() > 0;
function maybe(flag: boolean, cb: () => unknown) {
  if (flag) cb();
}

function underRoot(cb: () => unknown) {
  createRoot(() => cb());
}

// Violations: the helper runs the callback in its call, under no owner.
export function NullOwner() {
  runWithOwner(null, () => {
    call(() => onCleanup(() => {}));
  });
  return <div />;
}

export function Timer() {
  setTimeout(() => {
    call(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function Then() {
  Promise.resolve().then(() => {
    call(() => onCleanup(() => {}));
  });
  return <div />;
}

export function Apply() {
  createEffect(
    () => 1,
    () => {
      call(() => {
        onCleanup(() => {});
      });
    }
  );
  return <div />;
}

export function Imported() {
  setTimeout(() => {
    runNow(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function AlsoKeptTimer() {
  setTimeout(() => {
    callAndKeep(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

// Legal: the helper's call is owned.
export function Body() {
  call(() => onCleanup(() => {}));
  runNow(() => onCleanup(() => {}));
  return <div />;
}

export function InRoot() {
  setTimeout(() => {
    createRoot(() => {
      call(() => onCleanup(() => {}));
    });
  }, 0);
  return <div />;
}

export function MaybeBody() {
  maybe(on, () => onCleanup(() => {}));
  return <div />;
}

// Legal: the callback never runs, or runs under the helper's own root.
export function NeverCalledTimer() {
  setTimeout(() => {
    keep(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function OwnRootTimer() {
  setTimeout(() => {
    underRoot(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function ShadowedTimer() {
  const call = (cb: () => unknown) => {
    kept = cb;
  };
  setTimeout(() => {
    call(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

// Uncertifiable: a run this does not prove (on some paths only, or kept
// for a later run under an owner nothing names).
export function MaybeTimer() {
  setTimeout(() => {
    maybe(on, () => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function AlsoKeptBody() {
  callAndKeep(() => onCleanup(() => {}));
  return <div />;
}

// Violation: a dispatched handler has no owner (no synchronous dispatch here).
export function Handler() {
  return (
    <button
      onClick={() => {
        call(() => onCleanup(() => {}));
      }}
    >
      go
    </button>
  );
}

// ADR 0474 amendment: an owner guard and a hand-on to an owner-creating
// position. Legal: `fn()` runs only under an owner, else under the root.
function withOwner(fn: () => unknown) {
  return getOwner() ? fn() : createRoot(fn);
}

function inRoot(fn: () => unknown) {
  return createRoot(fn);
}

function bare(fn: () => unknown) {
  return !getOwner() ? fn() : createRoot(fn);
}

export function GuardTimer() {
  setTimeout(() => {
    withOwner(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function RootTimer() {
  setTimeout(() => {
    inRoot(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}

export function NegatedBody() {
  bare(() => onCleanup(() => {}));
  return <div />;
}

// Violation: `!getOwner()` holds in a timer, so `fn()` runs with no owner.
export function NegatedTimer() {
  setTimeout(() => {
    bare(() => onCleanup(() => {}));
  }, 0);
  return <div />;
}
