import { createEffect, createSignal, runWithOwner } from "solid-js";

const [count] = createSignal(0);

// Handed out as a value: App.tsx reads `member.handedOut` without calling it.
// Its detached callback runs with no owner whichever caller runs this function.
export function handedOut() {
  return runWithOwner(null, () => {
    createEffect(count, () => {});
    return 1;
  });
}

// Handed out as a value too. Its own body has no detached callback: an unknown
// caller may run it unowned, so the effect is uncertifiable, never a violation.
export function handedOutDirect() {
  createEffect(count, () => {});
}

// Called through the namespace by a component: its caller is known and owned.
export function setup() {
  createEffect(count, () => {});
}

// Nothing names it and nothing calls it: dormant in a closed program.
export function quiet() {
  return runWithOwner(null, () => {
    createEffect(count, () => {});
    return 1;
  });
}

// A named import used as a value, not read through the namespace: it keeps the
// closed program's dormancy, as a named value always did.
export function namedValue() {
  return runWithOwner(null, () => {
    createEffect(count, () => {});
    return 1;
  });
}

// Unexported and uncalled: dormant.
function unused() {
  return runWithOwner(null, () => {
    createEffect(count, () => {});
    return 1;
  });
}

void unused;
