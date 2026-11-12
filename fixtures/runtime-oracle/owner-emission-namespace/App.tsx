import * as ns from "./lib";

// ADR 0970, B2 of the review of #221: the program of fixtures/reactive-ir/owner-emission-layer-stops (`Namespace.tsx`) and its `lib.tsx`. The namespace of `./lib` is enumerated with
// no name: the timer calls every export with no owner, so Chrome rc.21 reports NO_OWNER_CLEANUP at the `onCleanup` of `Comp`; `Inner` is no export, only a tag renders it, and it is
// quiet. The owner emission certified `Comp` (the component stop read symbol references only: a namespace is no reference).
export function Host() {
  return <ns.Plain />;
}
setTimeout(() => {
  for (const component of Object.values(ns)) {
    (component as () => unknown)();
  }
}, 0);
