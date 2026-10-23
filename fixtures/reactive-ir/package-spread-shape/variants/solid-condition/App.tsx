import { onCleanup } from "solid-js";
import { attrs } from "cond-pkg";

// ADR 0372 under ADR 0297's premise: `cond-pkg`'s `solid` entry returns only
// `class`, its `default` entry also `onClick`. With a Solid Vite plugin
// installed (here the group's `fixtures/reactive-ir/node_modules`), the
// `solid` entry loads, the spread hands no handler and the ownerless
// `onCleanup` is a violation. Without one (`package_returns_process.rs`
// runs this from a copy outside the group), nothing says which entry loads,
// and the spread keeps its obligation.
export function Conditioned() {
  return <div onClick={() => onCleanup(() => {})} {...attrs()} />;
}
