import { createEffect, createMemo, createSignal, createStore, onSettled } from "solid-js";

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
