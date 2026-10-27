import { createContext, createEffect, createRoot, onCleanup, onSettled, useContext } from "solid-js";

const Ctx = createContext(0);

// ---- SC4001 (missing-owner). A call in a parameter list runs only when its
// function is called without the argument and in that function's activation:
// not in the module, nor in the handler or helper the function is written in.
// Nothing here proves the call, so these are uncertifiable, never violations.

export function HandlerCleanupDefault() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = onCleanup(() => {})) => m;
        void g;
      }}
    >
      x
    </button>
  );
}
export function HandlerEffectDefault() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = createEffect(() => 1, () => {})) => m;
        void g;
      }}
    >
      x
    </button>
  );
}
export function TimerDestructuredDefault() {
  setTimeout(() => {
    const g = ({ m = onCleanup(() => {}) }: { m?: unknown }) => m;
    void g;
  }, 0);
  return <div />;
}
export function MethodDefault() {
  return (
    <button
      onClick={() => {
        const o = { run(m: unknown = onCleanup(() => {})) { return m; } };
        void o;
      }}
    >
      x
    </button>
  );
}
export function ConstructorDefault() {
  return (
    <button
      onClick={() => {
        class K {
          constructor(public m: unknown = onCleanup(() => {})) {}
        }
        void K;
      }}
    >
      x
    </button>
  );
}
export function ForEachIndexSupplied() {
  return (
    <button
      onClick={() => {
        [1].forEach((v, i: unknown = onCleanup(() => {})) => void [v, i]);
      }}
    >
      x
    </button>
  );
}
export function ContextArgumentSupplied() {
  return (
    <button
      onClick={() => {
        const g = (m: number = useContext(Ctx)) => m;
        g(1);
      }}
    >
      x
    </button>
  );
}
function neverCalledLocal(m: unknown = onCleanup(() => {})) {
  return m;
}
function suppliedLocal(m: unknown = onCleanup(() => {})) {
  return m;
}
export function ModuleLevelDefaults() {
  return (
    <button
      onClick={() => {
        suppliedLocal(1);
        void neverCalledLocal;
      }}
    >
      x
    </button>
  );
}
function cleanupHelper() {
  onCleanup(() => {});
}
export function HelperReachedFromADefault() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = cleanupHelper()) => m;
        void g;
      }}
    >
      x
    </button>
  );
}
// Called without the argument in a handler: the default runs with no owner
// (rc.17 warns NO_OWNER_CLEANUP). Known miss: uncertifiable.
export function HandlerCallsOmitted() {
  return (
    <button
      onClick={() => {
        const g = (m: unknown = onCleanup(() => {})) => m;
        g();
      }}
    >
      x
    </button>
  );
}
// Called without the argument in the component body: owned, legal, silent.
export function BodyCallsOmitted() {
  const g = (m: unknown = onCleanup(() => {})) => m;
  g();
  return <div />;
}
// The component's own default runs in its owned body: silent.
export function ComponentOwnDefault(props: {}, m: unknown = onCleanup(() => {})) {
  return <div>{String(m)}</div>;
}
// A function literal invoked in the component's own list is legal (owned) and
// uncertifiable all the same, a cost: the invocation is an edge with no source.
export function ComponentOwnIifeDefault(props: {}, m: unknown = (() => onCleanup(() => {}))()) {
  return <div>{String(m)}</div>;
}
// Positive control: a cleanup directly in a handler body has no owner.
export function HandlerBody() {
  return (
    <button
      onClick={() => {
        onCleanup(() => {});
      }}
    >
      x
    </button>
  );
}

// ---- SC3001 (leaf-owner-forbidden-call). rc.17 calls an `onSettled` callback
// with no arguments, so a default in its own list runs in the leaf scope. The
// fact is the audited source's, not a dialect fact: uncertifiable, not silent.

export function LeafCallbackOwnDefault() {
  onSettled((x: unknown = onCleanup(() => {})) => {
    void x;
  });
  return <div />;
}
export function LeafBody() {
  onSettled(() => {
    onCleanup(() => {});
  });
  return <div />;
}
