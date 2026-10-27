import { createContext, createEffect, onCleanup, useContext } from "solid-js";

declare global {
  interface Window {
    ranA: number;
    ranB: number;
    ranC: number;
    ranD: number;
    ranE: number;
    ranF: number;
    ranG: number;
    ranH: number;
    ranI: number;
    ranJ: number;
    ranK: number;
  }
}
for (const key of ["ranA", "ranB", "ranC", "ranD", "ranE", "ranF", "ranG", "ranH", "ranI", "ranJ", "ranK"] as const) {
  window[key] = 0;
}

const Ctx = createContext(0);

// A closure with code in its parameter list is built in an event handler and
// never called: the default does not run, nothing is registered, rc.17 raises
// nothing.
export function Cleanup() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = (window.ranA++, onCleanup(() => {}))) => m;
        void g;
      }}
    >
      cleanup
    </button>
  );
}
export function Effect() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = (window.ranB++, createEffect(() => 1, () => {}))) => m;
        void g;
      }}
    >
      effect
    </button>
  );
}
export function Destructured() {
  setTimeout(() => {
    const g = ({ m = (window.ranC++, onCleanup(() => {})) }: { m?: unknown }) => m;
    void g;
  }, 0);
  return <div />;
}
export function Method() {
  return (
    <button
      onClick={() => {
        const o = { run(m: unknown = (window.ranD++, onCleanup(() => {}))) { return m; } };
        void o;
      }}
    >
      method
    </button>
  );
}
export function Constructor() {
  return (
    <button
      onClick={() => {
        class K {
          constructor(public m: unknown = (window.ranE++, onCleanup(() => {}))) {}
        }
        void K;
      }}
    >
      constructor
    </button>
  );
}
// `forEach` supplies the index, so the default never runs.
export function ForEachSupplied() {
  return (
    <button
      onClick={() => {
        [1].forEach((v, i: unknown = (window.ranF++, onCleanup(() => {}))) => void [v, i]);
      }}
    >
      foreach
    </button>
  );
}
// Called with the argument supplied: the default does not run.
export function ContextSupplied() {
  return (
    <button
      onClick={() => {
        const g = (m: number = (window.ranG++, useContext(Ctx))) => m;
        g(1);
      }}
    >
      context
    </button>
  );
}
function neverCalled(m: unknown = (window.ranH++, onCleanup(() => {}))) {
  return m;
}
function makeSupplied(m: unknown = (window.ranH++, onCleanup(() => {}))) {
  return m;
}
export function Module() {
  return (
    <button
      onClick={() => {
        makeSupplied(1);
        void neverCalled;
      }}
    >
      module
    </button>
  );
}
function cleanupHelper() {
  window.ranI++;
  onCleanup(() => {});
}
export function Interprocedural() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = cleanupHelper()) => m;
        void g;
      }}
    >
      helper
    </button>
  );
}
// Called without the argument in a handler: the default runs with no owner and
// rc.17 warns NO_OWNER_CLEANUP (known miss: uncertifiable).
export function CalledOmitted() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = (window.ranJ++, onCleanup(() => {}))) => m;
        g();
      }}
    >
      called
    </button>
  );
}
// Called without the argument in the component body: owned, legal.
export function OwnedBody() {
  const g = (m: unknown = (window.ranK++, onCleanup(() => {}))) => m;
  g();
  return <div />;
}
