import { createOwner, createSignal, runWithOwner } from "solid-js";
import { callNow } from "inline-helpers";
import callNowDefault from "default-helper";
import { keep } from "keep-helper";
import { later, whenScheduled } from "late-helper";
import { both } from "both-helper";

// ADR 0457, the same climb through dependency functions read from their source.
// The packages are synthetic, written for this fixture in published layout
// (`exports` with `types` and `default`, a `solid-js` peer): no version or
// integrity applies. Both files pass `tsc --noEmit` against `solid-js.d.ts` and
// against published rc.17 typings.
//
// Clean (rc.17 dev: the write lands): PackageNullReexport (the fuzzer's package
// shape, a re-export through impl.js), PackageNullDefault (a default export),
// PackageCreatedOwnerLater (`Promise.then`: a fresh stack, no owner).
// Violations (rc.17 throws): PackageCreatedOwnerReexport,
// PackageCreatedOwnerDefault.
// Uncertifiable: a package that keeps the callback (PackageNullKeeps,
// PackageCreatedOwnerKeeps) or runs it from a scheduler it does not name
// (PackageNullScheduled, PackageCreatedOwnerScheduled), or one that calls it in
// place and in a root it creates (PackageNullBoth; rc.17 throws in the root).

export function PackageNullReexport() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    callNow(() => {
      return setV(17);
    });
  });
  return <div>{v()}</div>;
}
export function PackageNullDefault() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    callNowDefault(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageCreatedOwnerReexport() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    callNow(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageCreatedOwnerDefault() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    callNowDefault(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageNullKeeps() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    keep(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageCreatedOwnerKeeps() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    keep(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageCreatedOwnerLater() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    later(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageNullScheduled() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    whenScheduled(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageCreatedOwnerScheduled() {
  const [v, setV] = createSignal(0);
  runWithOwner(createOwner(), () => {
    whenScheduled(() => setV(1));
  });
  return <div>{v()}</div>;
}
export function PackageNullBoth() {
  const [v, setV] = createSignal(0);
  runWithOwner(null, () => {
    both(() => setV(3));
  });
  return <div>{v()}</div>;
}
