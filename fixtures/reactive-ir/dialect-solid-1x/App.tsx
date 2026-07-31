import { createEffect, createMemo, createReaction, createSignal, onCleanup } from "solid-js";

// createStore is the only primitive both dialects have and export from
// different modules, which makes it the one name that can show this rule
// firing in both directions. Exactly one of these two lines is correct in each
// dialect, and it is the other one each time. The second is aliased because
// TypeScript will not take the same name twice -- which also exercises the
// `imported`-vs-`local` path, where the reported name is the one on the left
// of the `as`.
import { createStore } from "solid-js";
import { createStore as createStoreFromSubpath } from "solid-js/store";

// The 1.x/2.0 difference ADR 0001 led with, and the reason this fixture is
// duplicated rather than parameterised: the same source is correct under one
// dialect and a violation under the other.
//
// 1.x: createEffect(fn, value?) -- the callback is at argument 0 and argument
//      1 is a seed value, so this is a complete, correct effect.
// 2.0: createEffect(compute, apply) -- the callback is at argument 1, so this
//      effect has a compute and no side effect. SC7001.
export function OneArgument() {
  createEffect(() => {
    reader();
  });
  return null;
}

// Missing its effect function in both dialects, so both report SC7001 -- but
// they have to quote different signatures back at the reader. This is the
// case the wording matters for, which is why this project's snapshot keeps
// messages and hints.
export function NoArgument() {
  createEffect(undefined);
  return null;
}

// SC7002's hint names a boundary component, and the dialects spell it
// differently: <Suspense> in 1.x, <Loading> in 2.0.
export function SyncAsync() {
  return createMemo(async () => 1, { sync: true });
}

function reader() {
  return 1;
}

// createReaction is 1.x's leaf owner -- an owner whose callback ends the
// ownership chain, so onCleanup inside it is dropped. 2.0 does not treat it as
// one. Before the leaf set came from the dialect, the engine used 2.0's pair
// for both dialects and 1.x's leaf rules could not fire at all.
createReaction(() => {
  onCleanup(() => {});
});

// Names Solid 1.x exports and 2.0 removed. Each dialect gets its own bundled
// model of what solid-js exports, so these are ordinary imports under 1.x and
// names-that-do-not-exist under 2.0. Handing both dialects the same model --
// which is what happened until there was a 1.x contract -- reports every one
// of them against a correct 1.x project.
import { batch, onMount } from "solid-js";

export function OneXOnly() {
  onMount(() => {
    batch(() => {});
  });
}

// The second argument of createEffect, which is a different thing in each
// dialect and the same characters in both.
//
// 1.x: createEffect(fn, value?) -- argument 1 seeds `prev` and is evaluated
//      eagerly, here in SeedOrApply's body, which does not track. SC1001.
// 2.0: createEffect(compute, apply) -- argument 1 is the apply callback, a
//      legitimate place to read. Silent.
//
// The engine classified index 1 as an effect apply in both dialects until the
// apply position came from the dialect, so this read was reported under 1.x
// as living "in createEffect apply callback" -- a phase 1.x does not have.
export function SeedOrApply() {
  const [seed] = createSignal(1);
  createEffect(
    (prev) => prev ?? 0,
    (value) => {
      seed();
      return value;
    },
  );
  return null;
}
