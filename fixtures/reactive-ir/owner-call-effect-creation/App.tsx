import { createEffect, createMemo, createRoot } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, slice 6: a creation the dialect says throws PRIMITIVE_IN_EFFECT_CALLBACK on an effect's apply (`inEffectCallback && context === null`) is a site of the owner-call
// universe. It ends in SC6002's row, an SC9012, the compiler's discard, or a silence that names the fact the throw needs and the code lacks; with none it gets
// `effect-creation-unaccounted`, an uncertifiable row. Each case below is one of those.

// The apply creates a computation with no owner set: SC6002's own finding (the walk proves it), so no uncertifiable row stands beside it.
export function InApply() {
  createEffect(
    () => 0,
    () => {
      createMemo(() => 1);
    },
  );
  return <p>apply</p>;
}

// A root in the apply is the creation the guard meets (a violation); what the root's callback creates runs under the owner the root creates (`EffectCreationOwnerSetAround`).
export function RootInApply() {
  createEffect(
    () => 0,
    () => {
      createRoot(() => {
        createMemo(() => 2);
      });
    },
  );
  return <p>root</p>;
}

// A component body: rendering runs it under the owner its render sets (the role engine's `EffectCreationRunsOffApply`, or the graph's `OwnerCurrent`).
export function Body() {
  const memo = createMemo(() => 3);
  return <p>{memo()}</p>;
}

// An event handler: it runs on the browser's own stack, never on an effect's apply (no synchronous dispatch in this program).
export function Handler() {
  return (
    <button
      onClick={() => {
        createMemo(() => 4);
      }}
    >
      x
    </button>
  );
}

// A deferred callback runs on a fresh stack.
export function Timer() {
  setTimeout(() => {
    createMemo(() => 5);
  });
  return <p>timer</p>;
}

// Module scope: evaluation runs it once, on a stack of its own (`OwnerCallModuleScope`).
const top = createMemo(() => 6);

// A private function nothing references never runs (`OwnerCallNeverRuns`).
function dead() {
  return createMemo(() => 7);
}

// A helper stored in an array: it is entered by a reference that is no direct call, so nothing proves where it runs: an uncertifiable row.
const registry: Array<() => unknown> = [];
function stored() {
  return createMemo(() => 8);
}
registry.push(stored);

function App() {
  return (
    <p>
      <InApply />
      <RootInApply />
      <Body />
      <Handler />
      <Timer />
      {top()}
    </p>
  );
}
render(() => <App />, document.body);
