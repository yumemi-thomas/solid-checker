import { createEffect, createMemo, createSignal, createStore, onSettled, resolve, until } from "solid-js";

// ADR 1166: a read in the body of a `const`-bound callback is read untracked where a run of the callback provably executes it
// there: a direct call or a literal `forEach`/`map` carrier, written after the declarator in the function that declares it,
// unconditionally, at a position where a read written in its place reports. The callback's other entries do not matter.

// Violations: a reporting run in the body or an effect's apply.
export function StoreMemoCalledInBody() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  f();
  return <div>{m()}</div>;
}

export function StoreMemoCarriedInBody() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  [f].forEach((g) => g());
  return <div>{m()}</div>;
}

export function StoreMemoMappedInBody() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  [f].map((g) => g());
  return <div>{m()}</div>;
}

export function StoreSettledCarriedInBody() {
  const [s] = createStore({ v: 0 });
  const f = () => {
    void s.v;
  };
  onSettled(f);
  [f].forEach((g) => g());
  return <div />;
}

export function StoreEffectComputeCarriedInBody() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  createEffect(f, () => {});
  [f].forEach((g) => g());
  return <div />;
}

export function StoreNamedExpressionCarriedInApply() {
  const [s] = createStore({ v: 0 });
  createEffect(
    () => 1,
    () => {
      const f = function inner() {
        return s.v;
      };
      [f].forEach((g) => g());
      createMemo(f);
    },
  );
  return <div />;
}

// Uncertifiable: in an apply, the creation before the run throws PRIMITIVE_IN_EFFECT_CALLBACK first (rc.21), so the run never comes.
export function TwinApplyCreationFirst() {
  const [s] = createStore({ v: 0 });
  createEffect(
    () => 1,
    () => {
      const f = function inner() {
        return s.v;
      };
      createMemo(f);
      [f].forEach((g) => g());
    },
  );
  return <div />;
}

export function SignalMemoCarriedInBody() {
  const [t] = createSignal(1);
  const f = () => t();
  const m = createMemo(f);
  [f].forEach((g) => g());
  return <div>{m()}</div>;
}

// Clean: the only other run is in a click handler, where no read rule reports (`map` is a carrier the entry census counts).
export function StoreMemoMappedInHandler() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  return <button onClick={() => [f].map((g) => g())}>{m()}</button>;
}

// Uncertifiable: no run is proven to execute the read where it reports.
export function TwinDeadCall() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  if (false) f();
  return <div>{m()}</div>;
}

export function TwinConditionalCarrier(props: { on: boolean }) {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  [f].forEach((g) => {
    if (props.on) g();
  });
  return <div>{m()}</div>;
}

export function TwinLoopCall() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  for (const x of [1, 2]) {
    void x;
    [f].forEach((g) => g());
  }
  return <div>{m()}</div>;
}

export function TwinFilter() {
  const [s] = createStore({ v: 0 });
  const f = () => s.v;
  const m = createMemo(f);
  [f].filter((g) => g());
  return <div>{m()}</div>;
}

export function TwinLetBinding() {
  const [s] = createStore({ v: 0 });
  let f = () => s.v;
  const m = createMemo(f);
  f = () => 0;
  [f].forEach((g) => g());
  return <div>{m()}</div>;
}

export function TwinNestedRead() {
  const [s] = createStore({ v: 0 });
  const f = () => {
    const h = () => s.v;
    return h;
  };
  const m = createMemo(f);
  [f].forEach((g) => g());
  return <div>{m()()}</div>;
}

// Uncertifiable (SC1003 too): a destructure in a callback a non-literal list carries; no run is proven.
export function TwinDestructureListed() {
  const [s] = createStore({ v: 0 });
  const f = () => {
    const { v } = s;
    return v;
  };
  const m = createMemo(f);
  const l = [f];
  l.forEach((g) => g());
  return <div>{m()}</div>;
}

// ADR 1166 r2 (census false cleans `barm-carried-body`): `resolve`/`until` in a function an effect's compute runs tracked and a
// carried `forEach` runs untracked. The compute run throws (RESOLVE/UNTIL_IN_REACTIVE_SCOPE); the untracked run proves nothing
// about it, so SC2004/SC2005 keep their uncertifiable row (`function-run-unplaced`), never silent.
export function TwinResolveComputeCarried() {
  const f1 = () => {
    resolve(() => 1);
  };
  createEffect(f1, () => {});
  [f1].forEach((g) => g());
  return <div />;
}

export function TwinUntilComputeCarried() {
  const f1 = () => {
    until(() => 1);
  };
  createEffect(f1, () => {});
  [f1].forEach((g) => g());
  return <div />;
}
