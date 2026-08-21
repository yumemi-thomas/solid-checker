// A function's caller-visible read summary is about its *synchronous extent*.
// Calling a helper performs only the reads that happen while the call runs, so
// a read sealed inside a callback the primitive runs later, or in its own
// tracking scope, is not the caller's read and must not be attributed to it.
//
// The dialect's own callback vocabulary decides this, and its three
// executions divide cleanly:
//
//   Inline   -- "reads inside it subscribe whatever was tracking at the call
//               site", so the read IS the caller's and propagates.
//   Tracked  -- reads subscribe the callback's own observer.
//   Deferred -- reads subscribe nothing the caller owns.
//
// Only the first propagates. Previously the summary excluded just one shape
// (2.0's `createEffect` apply slot, matched by name and Deferred), so a 1.x
// effect -- whose callback is Tracked -- leaked its read into the enclosing
// helper's summary. Calling such a helper from a render scope then produced a
// *proven* SC1001 violation for a read that never happens at the call site,
// while the same read inside the helper was correctly silent. The two halves
// of the analyzer disagreed, and the interprocedural half was wrong.
//
// Every helper is local and every call site sits under createRoot, so the
// owner question is settled and this fixture reports on one claim only.
import { createEffect, createMemo, createRoot, createSignal, onMount, untrack } from "solid-js";

const [count] = createSignal(0);

// Tracked callback: the read subscribes the effect, not the caller.
function effectOnly() {
  createEffect(() => { sink(count()); });
}

// Tracked callback: the read subscribes the memo.
function memoOnly() {
  return createMemo(() => count() * 2);
}

// Deferred callback: the read runs after the call returns.
function mountOnly() {
  onMount(() => { sink(count()); });
}

// Inline callback: untrack runs its callback during the call, so the read
// happens in the caller's execution and still propagates. This is the case
// that keeps the fix from being "ignore every callback".
function untrackedNow() {
  return untrack(() => count());
}

// The plain control: the read is in the helper's own body.
function readsDirectly() {
  return count();
}

// An eagerly evaluated argument is not a callback at all. `count()` is read
// while the argument list is built -- `compute` merely receives the result --
// so this propagates too, even though the slot it sits in is onMount's
// Deferred callback slot. That is why the proof requires a function literal
// between the read and the argument rather than just an enclosing callback
// slot: keyed on the slot alone, this read would be wrongly discarded.
function eagerArgument() {
  onMount(compute(count()));
}

// Reading in a render scope is what turns a propagated read into a finding,
// so every case above is exercised from one place. Host is rendered at an
// exact JSX call site below, which proves its component identity under 1.x --
// so it is a proven owner for the helpers and a scope that does not track for
// the reads. That keeps the owner question out of this fixture entirely and
// leaves it reporting on one claim.
function Host() {
  effectOnly();
  memoOnly();
  mountOnly();
  untrackedNow();
  readsDirectly();
  eagerArgument();
  return <div />;
}

createRoot(() => <Host />);

declare function sink(value: number): void;
declare function compute(seed: number): () => number;
