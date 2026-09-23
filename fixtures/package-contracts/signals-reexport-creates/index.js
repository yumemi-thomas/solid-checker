// Every callee is imported from `solid-js`, and four of them resolve through
// its re-export into `@solidjs/signals`, as they do in the published archive.
// The `creates` walk asks the audits about the package that *declares* a
// callee, so this is the path on which the `@solidjs/signals` rows are reached.
import {
  createContext,
  createTrackedEffect,
  getOwner,
  onCleanup,
  runWithOwner,
  useContext
} from "solid-js";

// `createTrackedEffect`'s `creates` is audited closed, so the walk clears this
// export; its call registers a computation on the caller's owner, which is the
// `Effect` requirement this generation withholds. `creates` must stay open: a
// consumer reads a closed `creates` as "no owner requirement".
export function trackEach(handle) {
  createTrackedEffect(() => handle());
}

// The control: `onCleanup`'s requirement is published as a `cleanups` item, so
// the closure states everything and `creates` is proposed.
export function cleanUp(handle) {
  onCleanup(() => handle());
}

// The 2026-09-23 owner-and-context audit's three rows.
export function runUnder(owner, fn) {
  return runWithOwner(owner, fn);
}

export function makeContext(value) {
  return createContext(value);
}

export function readContext(context) {
  return useContext(context);
}

// `getOwner` has a row too. A returned call is demanded a resolved call, so the
// walk can tell which package declares it here.
export function currentOwner() {
  return getOwner();
}

// Anywhere else, a call with no argument had no resolved call: the walk could
// not tell which package declares `getOwner`, and declined as the dialect's
// silence. `@solid-primitives/utils`' `tryOnCleanup` is this shape.
export function hasOwner() {
  return getOwner() !== null;
}
