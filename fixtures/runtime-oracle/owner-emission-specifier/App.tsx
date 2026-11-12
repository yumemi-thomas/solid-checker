import * as ns from "./lib";

// ADR 0970, R1 and N1 of the re-reviews of #221: the program of fixtures/reactive-ir/owner-emission-layer-stops (`Namespace.tsx`) with its `lib.tsx` written as export specifiers. The
// namespace of `./lib` is enumerated with no name: the timer calls every export with no owner, so Chrome rc.21 reports NO_OWNER_CLEANUP at the `onCleanup` of `Spec`, `Aliased`,
// `Arrow`, `Named` and `Same` (the last two a `const` bound to a function expression that carries a name of its own); `Inner` is no export, only a tag renders it, and it is quiet.
// The owner emission certified the first three on b80d5ad57 (its export test wanted the export statement to contain the function) and the last two on 03bfe0630 (it resolved the
// function to the name the expression carries, not to the `const` the specifier names).
export function Host() {
  return <ns.Plain />;
}
setTimeout(() => {
  for (const component of Object.values(ns)) {
    (component as () => unknown)();
  }
}, 0);
