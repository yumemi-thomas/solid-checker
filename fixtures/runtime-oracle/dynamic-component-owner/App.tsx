import { createEffect, onCleanup } from "solid-js";
import { Dynamic } from "@solidjs/web";

// ADR 0992: `Dynamic` calls the function it gets in `createMemo(() => … untrack(() => component(props)))`, so `Chosen`'s cleanup has an owner: Chrome reports nothing and the checker is
// silent. `Kept` is also stored and called from an effect's apply, where no owner is current (NO_OWNER_CLEANUP): the checker must answer there, never stay silent.
function Chosen() {
  onCleanup(() => {});
  return <p>chosen</p>;
}

let saved: (() => unknown) | undefined;
function Kept() {
  onCleanup(() => {});
  return <p>kept</p>;
}

export function ByDynamic() {
  return <Dynamic component={Chosen} />;
}

export function ByKept() {
  saved = Kept;
  createEffect(
    () => 0,
    () => {
      saved!();
    },
  );
  return <Dynamic component={Kept} />;
}

// `Dynamic` called with no owner current (a timer): its memo is the owner of what it renders, so `Chosen`'s cleanup has an owner. Chrome reports nothing.
export function ByTimer() {
  setTimeout(() => {
    (Dynamic as unknown as (props: object) => unknown)({ component: Chosen });
  }, 0);
  return <p>Dynamic called from a timer</p>;
}
