import { createSignal, createStore } from "solid-js";

// ADR 0323: a props or store read inside an anonymous callback is classified
// like a signal read in the same callback, but only where the callback reaches
// a project callee, an inline standard-library callback or an IIFE. Every
// component below that must be a *proven* violation has one caller (`Root`)
// that passes a signal-backed prop, so its props are getters.

type Opts = { onClose: () => void };

// A project callee that calls its parameter in its own body.
function now(f: () => void) {
  f();
}
// A project callee that calls a member of its options parameter.
function useThing(o: Opts) {
  o.onClose();
}
// A project callee that hands the callback to a timer.
function later(f: () => void) {
  setTimeout(f, 0);
}

// (i) project callee, direct argument that the callee calls in its body:
// proven, the callback runs while the body runs.
function ProjectDirect(props: { x: number }) {
  now(() => console.log(props.x));
  return <div />;
}
// (i) project callee, options-object property the callee calls through a
// member: not proven to run in the body (ADR 0321 resolves the dispatch, not
// its timing), so uncertifiable like the signal twin.
function ProjectOptions(props: { x: number }) {
  useThing({ onClose: () => console.log(props.x) });
  return <div />;
}
// (i) project callee that defers the callback: the flow role is a timer's,
// nothing is reported.
function ProjectDeferred(props: { x: number }) {
  later(() => console.log(props.x));
  return <div />;
}
// (ii) inline standard-library callbacks.
function StdlibForEach(props: { k: number }) {
  [1, 2].forEach((x) => console.log(x + props.k));
  return <div />;
}
function StdlibMap(props: { k: number }) {
  const ys = [1, 2].map((x) => x + props.k);
  return <div>{ys.length}</div>;
}
// (iii) IIFE.
function Iife(props: { x: number }) {
  (() => console.log(props.x))();
  return <div />;
}
// Store reads, same shapes.
function StoreProject() {
  const [s] = createStore({ a: 1 });
  now(() => console.log(s.a));
  return <div />;
}
function StoreStdlib() {
  const [s] = createStore({ a: 1 });
  [1].forEach((x) => console.log(x + s.a));
  return <div />;
}
function StoreIife() {
  const [s] = createStore({ a: 1 });
  (() => console.log(s.a))();
  return <div />;
}
// A nested layer that is modelled all the way up: an IIFE inside a project
// callee's callback.
function NestedModelled(props: { x: number }) {
  now(() => {
    (() => console.log(props.x))();
  });
  return <div />;
}

// ADR 0323 (review round 9): a const-bound helper called directly in an
// admitted callback is a layer of its own, as it is in the component body.
function NestedHelper(props: { x: number }) {
  now(() => {
    const g = () => props.x;
    console.log(g());
  });
  return <div />;
}

declare function sink(x: unknown): void;

// Review round 10: a helper that is kept (an alias, a store) inside an admitted
// callback may run in the render window through the reference that keeps it:
// an obligation, never silent. Passing it to unknown code stays option B's
// silence (`Silent.tsx`).
function HelperAliased(props: { x: number }) {
  now(() => {
    const g = () => props.x;
    const h = g;
    sink(h());
  });
  return <div />;
}
function HelperStored(props: { x: number }) {
  let saved: (() => number) | undefined;
  now(() => {
    const g = () => props.x;
    saved = g;
  });
  sink(saved?.());
  return <div />;
}

// An exported component keeps its callers unenumerable: the same read is an
// uncertifiable obligation, not a violation.
export function ExportedDirect(props: { x: number }) {
  now(() => console.log(props.x));
  return <div />;
}

export function Root() {
  const [n] = createSignal(1);
  return (
    <div>
      <ProjectDirect x={n()} />
      <ProjectOptions x={n()} />
      <ProjectDeferred x={n()} />
      <StdlibForEach k={n()} />
      <StdlibMap k={n()} />
      <Iife x={n()} />
      <StoreProject />
      <StoreStdlib />
      <StoreIife />
      <NestedModelled x={n()} />
      <NestedHelper x={n()} />
      <HelperAliased x={n()} />
      <HelperStored x={n()} />
    </div>
  );
}
