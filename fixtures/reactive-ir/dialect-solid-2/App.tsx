import { createEffect, createMemo, createReaction, createSignal, onCleanup } from "solid-js";

// createStore is the one name both dialects have and export from different
// modules. Under 1.x the root import on the first line is wrong -- createStore
// lives in solid-js/store -- and SC8002 (v1/imports) says so, while the
// subpath import is correct. The imports rule is v1-only, so 2.0 reports
// neither line here; its wrong-module coverage is the contract rules, which
// OneXOnly below exercises. The second import is aliased because TypeScript
// will not take the same name twice -- which also exercises the
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

// The same async computation is a different defect in each dialect. 1.x has
// no sync option, so the async createMemo scope is SC5004 and the hint points
// at createResource. 2.0 models { sync: true } and reports SC7002, whose hint
// names <Loading>.
export function SyncAsync() {
  return createMemo(async () => 1, { sync: true });
}

function reader() {
  return 1;
}

// createReaction is 1.x's leaf owner -- an owner whose callback ends the
// ownership chain, so onCleanup inside it is dropped. 2.0 does not treat it as
// one, but the invalidation callback is still unowned there: onCleanup inside
// it reports no-owner-cleanup, matching the runtime's NO_OWNER_CLEANUP. Before the leaf set came from the dialect, the engine used 2.0's pair
// for both dialects and 1.x's leaf rules could not fire at all.
const [reactionDependency, setReactionDependency] = createSignal(0);
const trackReaction = createReaction(() => {
  onCleanup(() => {});
});
trackReaction(() => reactionDependency());
setReactionDependency(1);

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
// 1.x: createEffect(fn, value?) -- argument 1 is a function-valued seed. Its
//      body is never invoked by createEffect, so the read is dormant. Silent.
// 2.0: createEffect(compute, apply) -- argument 1 is the apply callback. It
//      runs, and it does not track: the seed() read is SC1001, and the
//      `return value` cannot be proven cleanup-or-undefined, so SC9002.
//
// The engine classified index 1 as an invoked effect apply in both dialects
// until callback execution came from the concrete dialect call contract.
// A dormant seed contributes neither reachability nor reactive-read facts.
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

// A constant-foldable value on a specially-spelled attribute. The compiler
// folds these while its census names the sites from the spelling alone, a
// disagreement that once made this whole file unanalysable ("semantic trace
// has unresolved execution sites"). Nothing here is reactive and nothing
// should be reported; the fixture exists so a compiler regression fails
// analysis loudly under both dialects.
export function FoldedSpecialAttributes() {
  const folded = "static";
  return (
    <section>
      <div ref={folded} />
      <div children={folded} />
    </section>
  );
}

// Solid 1.x stores a function passed to createSignal as the signal's value;
// it does not execute it as a derived computation. Solid 2.0 deliberately
// differs, but both versions must stay silent here: in 1.x the function is
// dormant, and in 2.0 the derived computation tracks the read.
export function StoredFunctionValue() {
  const [storedFunctionSource] = createSignal(1);
  createSignal(() => storedFunctionSource());
  return null;
}

// `sync` is a 2.0 runtime option. A 1.x analysis must not emit a diagnostic
// owned only by the 2.0 rule catalog, even when invalid declarations expose
// the same object shape to the AST.
export function TwoOnlySyncOption() {
  createMemo(async () => 1, undefined, { sync: true });
  return null;
}
