import * as ns from "./lib";

// ADR 0970, R1 of the re-review of #221: the program of fixtures/reactive-ir/owner-emission-layer-stops (`Namespace.tsx`) with its `lib.tsx` written as export specifiers. The
// namespace of `./lib` is enumerated with no name: the timer calls every export with no owner, so Chrome rc.21 reports NO_OWNER_CLEANUP at the `onCleanup` of `Spec`, `Aliased`
// and `Arrow`; `Inner` is no export, only a tag renders it, and it is quiet. The owner emission certified the three (its export test wanted the export statement to contain the
// function).
export function Host() {
  return <ns.Plain />;
}
setTimeout(() => {
  for (const component of Object.values(ns)) {
    (component as () => unknown)();
  }
}, 0);
