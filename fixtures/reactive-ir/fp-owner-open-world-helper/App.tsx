import { createEffect, onCleanup } from "solid-js";

// Uncertifiable: an exported non-component function may be called by code
// outside the project, with or without an owner.
export function exportedRegister() {
  onCleanup(() => {});
}

// Uncertifiable, one call edge away: the private helper is reached only from
// exported functions, so its caller set is just as open as theirs.
function privateRegister() {
  onCleanup(() => {});
}

export function exportedCaller() {
  privateRegister();
}

// Violation: the helper is called from a createEffect apply callback, which
// runs with no owner, so the cleanup is proven unowned.
function registerFromApply() {
  onCleanup(() => {});
}

export function ApplyCaller() {
  createEffect(
    () => 1,
    () => {
      registerFromApply();
    },
  );
  return <div />;
}
