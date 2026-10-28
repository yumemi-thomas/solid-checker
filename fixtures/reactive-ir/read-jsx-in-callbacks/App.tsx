import * as S from "solid-js";
import {
  createEffect,
  createRoot,
  createSignal,
  createStore,
  getOwner,
  runWithOwner,
  untrack,
} from "solid-js";

// ADR 0470: JSX the compiler wraps runs in its own render computation (or a
// getter), wherever the JSX is evaluated. rc.17 silent for every case here.

function K(props: { v?: number; children?: number }) {
  return (
    <i>
      {props.v}
      {props.children}
    </i>
  );
}

export function RootChild() {
  const [r] = createSignal(0);
  createRoot(() => <div>{r()}</div>);
  return <p>x</p>;
}

export function RootAttribute() {
  const [r] = createSignal(0);
  createRoot(() => <div title={String(r())} />);
  return <p>x</p>;
}

export function RootNamespaceChild() {
  const [r] = S.createSignal(0);
  S.createRoot(() => <div>{r()}</div>);
  return <p>x</p>;
}

export function RootStoreMemberChild() {
  const [s] = createStore({ a: 1 });
  createRoot(() => <div>{s.a}</div>);
  return <p>x</p>;
}

export function RootComponentProp() {
  const [r] = createSignal(0);
  createRoot(() => <K v={r()} />);
  return <p>x</p>;
}

export function RootComponentChild() {
  const [r] = createSignal(0);
  createRoot(() => <K>{r()}</K>);
  return <p>x</p>;
}

export function RootIifeInChild() {
  const [r] = createSignal(0);
  createRoot(() => <div>{(() => r())()}</div>);
  return <p>x</p>;
}

export function RootForEachChild() {
  const [r] = createSignal(0);
  createRoot(() => {
    [1].forEach(() => {
      const el = <div>{r()}</div>;
    });
  });
  return <p>x</p>;
}

export function OwnerChild() {
  const [r] = createSignal(0);
  const o = getOwner();
  runWithOwner(o, () => <div>{r()}</div>);
  return <p>x</p>;
}

export function UntrackChild() {
  const [r] = createSignal(0);
  const el = untrack(() => <div>{r()}</div>);
  return <p>{el}</p>;
}

// rc.17 throws PRIMITIVE_IN_EFFECT_CALLBACK at the insert the JSX creates in
// the apply; the read itself never runs untracked.
export function ApplyChild() {
  const [r] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      const el = <div>{r()}</div>;
    }
  );
  return <p>x</p>;
}

export function HandlerChild() {
  const [r] = createSignal(0);
  return (
    <button
      onClick={() => {
        const el = <div>{r()}</div>;
      }}
    >
      x
    </button>
  );
}

// Positive controls (rc.17 warns STRICT_READ_UNTRACKED): the read beside the
// JSX, in the callback's own body.
export function RootBodyRead() {
  const [r] = createSignal(0);
  createRoot(() => {
    r();
    return <div />;
  });
  return <p>x</p>;
}

export function RootBodyBesideJsx() {
  const [r] = createSignal(0);
  createRoot(() => {
    const el = <div class={r() ? "a" : "b"}>{r() + 1}</div>;
    r();
    return el;
  });
  return <p>x</p>;
}

export function OwnerBodyBesideJsx() {
  const [r] = createSignal(0);
  const o = getOwner();
  runWithOwner(o, () => {
    r();
    return <div>{r()}</div>;
  });
  return <p>x</p>;
}

export function ApplyBodyRead() {
  const [r] = createSignal(0);
  createEffect(
    () => 1,
    () => {
      r();
    }
  );
  return <p>x</p>;
}
