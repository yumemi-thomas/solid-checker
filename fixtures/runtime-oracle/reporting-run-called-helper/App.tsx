import { createStore, flush, onSettled } from "solid-js";

// ADR 1189: ADR 1166's reporting run gives a read in a named callback the role of a read written at the callback's run. That role is
// the read's own only where the function holding the run is entered by nobody but the runtime (a component body, a primitive's
// callback), or by direct calls written where that holds in turn. A function handed on as a value (`[1].map(Row)`) may not run as a
// component, so a run in it, or in a helper it calls, proves nothing: the read stays uncertifiable.

// The census false positive (lg-strict-read-untracked-handed-flush-store-*): `Row` is called by `map` inside a JSX expression, a
// tracked computation, so the read is tracked there (Chrome: silent). Main claimed SC1001 from the helper's rendering role.
export function MappedRow() {
  const [s1] = createStore({ v: 0 });
  const wrapUse = () => {
    const f = () => {
      void s1.v;
    };
    onSettled(f);
    [f].forEach((g) => g());
  };
  const Row = () => {
    flush(() => {
      wrapUse();
    });
    return <span />;
  };
  return (
    <p>
      {(() => {
        [1].map(Row);
        return 1;
      })()}
    </p>
  );
}

// A helper the component body calls directly: its run is the body's, untracked (SC1001 a violation; Chrome warns).
export function BodyCalledHelper() {
  const [s1] = createStore({ v: 0 });
  const use = () => {
    const f = () => {
      void s1.v;
    };
    onSettled(f);
    [f].forEach((g) => g());
  };
  use();
  return <i />;
}

// The same through a function declaration and through an immediately invoked arrow (violations).
export function DeclaredHelper() {
  const [s1] = createStore({ v: 0 });
  function use() {
    const f = () => {
      void s1.v;
    };
    onSettled(f);
    [f].forEach((g) => g());
  }
  use();
  return <i />;
}
export function InvokedHelper() {
  const [s1] = createStore({ v: 0 });
  (() => {
    const f = () => {
      void s1.v;
    };
    onSettled(f);
    [f].forEach((g) => g());
  })();
  return <i />;
}

// The callback declared in the component body itself: its run's role is the body's (SC1001 a violation, Chrome warns).
export function BodyDeclared() {
  const [s1] = createStore({ v: 0 });
  const f = () => {
    void s1.v;
  };
  onSettled(f);
  [f].forEach((g) => g());
  return <i />;
}

// The callback declared and run in `Row` itself, which `map` calls as a plain function inside a JSX expression (tracked there):
// uncertifiable, Chrome silent.
export function RowDeclared() {
  const [s1] = createStore({ v: 0 });
  const Row = () => {
    const f = () => {
      void s1.v;
    };
    onSettled(f);
    [f].forEach((g) => g());
    return <span />;
  };
  return (
    <p>
      {(() => {
        [1].map(Row);
        return 1;
      })()}
    </p>
  );
}
