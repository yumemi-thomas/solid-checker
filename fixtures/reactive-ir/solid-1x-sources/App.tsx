import { createComputed, createDeferred, createEffect, createMemo, createResource, createSelector, createSignal, For, Index } from "solid-js";
import { createMutable } from "solid-js/store";

// The reactive-source factories 1.x has and 2.0 does not. A read the engine
// cannot trace to a source is a read it reports nothing about, so each of
// these untracked reads is evidence that the source was discovered at all.
//
// This only works because `node_modules/solid-js/package.json` pins a 1.x
// version -- dialect selection reads it (see solid-facts-backend's dialect.rs).
// That file was an *empty directory* until 2026-08-17, so the fixture had been
// running the 2.0 catalog the whole time and every claim below was vacuous:
// the 1.x-only names produced six `package-contract-incomplete` findings
// obligations and the single-argument `createEffect` drew a spurious
// `missing-effect-function`. The `.gitignore` exception lines for that
// directory are part of the fixture -- without them the stub is untracked and
// the fixture silently un-dialects in CI, which is the trap AGENTS.md records.
//
// Two shapes, two mechanisms. createResource returns a tuple, which the
// bundled contract's single-value `returns` column cannot describe, so
// Dialect::creates_reactive_source answers for it. createDeferred,
// createSelector and createMutable return one value and are described by the
// contract. Before both were per-dialect, the engine used 2.0's list and none
// of these was found.
export function Reads() {
  const [count] = createSignal(0);
  const memo = createMemo(() => count());
  const [data] = createResource(async () => 1);
  const deferred = createDeferred(() => count());
  const selected = createSelector(count);
  const mutable = createMutable({ a: 1 });
  mutable.a = 2;

  console.log(count(), memo(), data(), deferred(), selected(1), mutable.a);
  return <div />;
}

// 1.x renders <Index each>{item => ...}</Index> the way 2.0 renders <Repeat>.
// The engine's control-flow set named Repeat, which 1.x does not have, and
// omitted Index, which it does -- so a function written inside an <Index> was
// read as a component and the untracked read in its body went unreported.
// Both of these must report; only the <For> one did.
export function ControlFlow() {
  const [items] = createSignal([1, 2]);
  const [count] = createSignal(0);
  return (
    <div>
      <For each={items()}>{(item) => { const v = count(); return <div>{item}{v}</div>; }}</For>
      <Index each={items()}>{(item) => { const v = count(); return <div>{item()}{v}</div>; }}</Index>
    </div>
  );
}

// 1.x declares its effect callbacks as `(prev: Prev) => Next` and threads the
// return value to the next run, so this accumulating form is idiomatic and the
// returned number is data, not a cleanup. Returning a cleanup is a 2.0 idea;
// the shared list that said otherwise named createEffect, and reported this.
export function Accumulates() {
  const [count] = createSignal(0);
  createEffect((prev: number) => prev + count(), 0);
}

// A tracked 1.x computation that 2.0 does not have. The read after the await
// runs without the subscription the synchronous part had, which is SC1002.
//
// The set of "computations whose reads matter after an await" was a hardcoded
// eight, and it was 2.0's: `createComputed` is not in 2.0 and so was in no
// list, even though `docs/solid-1x-api-surface.md` names it alongside
// createMemo and createEffect as one of 1.x's tracked computations.
export function AwaitsInsideComputed() {
  const [count] = createSignal(0);
  createComputed(async () => {
    await Promise.resolve();
    return count();
  });
}

// <For> and <Index> hand their children callback exactly opposite shapes, and
// 1.x's flow.d.ts says so:
//
//   For:   (item: T[number],           index: Accessor<number>)
//   Index: (item: Accessor<T[number]>, index: number)
//
// Both untracked reads below must report. Only the <For> one did: the engine
// had <For>'s answer hardcoded and no <Index> arm at all, so `item` inside
// <Index> was not a reactive source and a read of it was traced to nothing.
export function MirroredParameters() {
  const [items] = createSignal([1, 2]);
  return (
    <div>
      <Index each={items()}>{(item) => { const v = item(); return <div>{v}</div>; }}</Index>
      <For each={items()}>{(item, index) => { const i = index(); return <div>{item}{i}</div>; }}</For>
    </div>
  );
}
