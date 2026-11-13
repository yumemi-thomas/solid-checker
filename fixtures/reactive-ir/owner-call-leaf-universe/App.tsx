import { createMemo, createTrackedEffect, onCleanup } from "solid-js";
import * as S from "solid-js";
import { render } from "@solidjs/web";

// The program opens a leaf scope (`createTrackedEffect`), so the program census does not answer for any call below: each is asked of the owner graph.

export function Opener() {
  createTrackedEffect(() => {
    // The leaf callback creates a computation itself: SC3001's own finding stands here, so no uncertifiable row stands beside it.
    createMemo(() => 1);
  });
  return <p>opener</p>;
}

// A helper only a component's body calls: its context is owned, holds no leaf bit, and every entry into it is a call the graph holds.
function useDouble() {
  const double = createMemo(() => 2);
  onCleanup(() => {});
  return double;
}
export function Plain() {
  const double = useDouble();
  return <p>{double()}</p>;
}

// The same kind of helper also called from a leaf callback: its context has a leaf bit, so the silence does not answer; the leaf walk's row stands at the creation.
function useBoth() {
  return createMemo(() => 3);
}
export function Both() {
  createTrackedEffect(() => {
    useBoth();
  });
  return <p>{useBoth()()}</p>;
}

// A helper stored in an array: it is entered by a reference that is no direct call, so the graph holds no edge for that entry and a package or later code may run it under a
// leaf owner. Its calls keep their uncertifiable rows.
const registry: Array<() => unknown> = [];
function stored() {
  const kept = createMemo(() => 4);
  onCleanup(() => {});
  return kept;
}
registry.push(stored);

// The namespace spelling of the primitive is the same site as the named import's.
function viaNamespace() {
  return S.createMemo(() => 5);
}
export function Namespaced() {
  return <p>{viaNamespace()()}</p>;
}

// A local function named like the primitive is no site.
function shadowed() {
  function createMemo(compute: () => number) {
    return compute;
  }
  return createMemo(() => 6);
}
export function Shadowing() {
  return <p>{shadowed()()}</p>;
}

// The root: each component above is rendered from here, so each has an owner edge.
function App() {
  return (
    <p>
      <Opener />
      <Plain />
      <Both />
      <Namespaced />
      <Shadowing />
    </p>
  );
}
render(() => <App />, document.body);
