import { For, Show, createContext, createEffect } from "solid-js";

// B2 (the review of #225, the in-place rule): a function handed to a dialect control-flow element is called by that element's own render, under the owner it sets, and its
// getters (a child expression, a prop's value) are read there, so a creation WRITTEN IN THEIR BODY runs in place. A function written INSIDE one of them is not in place: it runs where
// it is called (here, stored in a variable and called by an effect's apply, with no owner set), so the tag in it is a site like any other. Chrome rc.21 throws
// PRIMITIVE_IN_EFFECT_CALLBACK at `<Inner />` (createComponent runs its component under createRoot, which refuses an apply's stack with no owner) in every export below but the
// control. The rule as first built took every site inside the handed function, or inside an in-place getter, for one it runs in place, and the checker certified all of them.
// `Rows`, `Shown` and the other helpers are written outside the exported components, which only a tag renders, so B3 (ADR 0967) keeps no row at their sites.

function Inner() {
  return <p>inner</p>;
}

// A function written inside the function handed to `For`.
let pickRow: (() => unknown) | undefined;

function Rows() {
  return (
    <For each={[1]}>
      {(item) => {
        const view = () => <Inner />;
        pickRow = view;
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ByRow() {
  createEffect(
    () => 0,
    () => {
      pickRow!();
    },
  );
  return (
    <p>
      <Rows />
    </p>
  );
}

// ... inside the function child of `Show`.
let pickShow: (() => unknown) | undefined;

function Shown() {
  return (
    <Show when={1}>
      {(value) => {
        const view = () => <Inner />;
        pickShow = view;
        return <p>{String(value())}</p>;
      }}
    </Show>
  );
}

export function ByShow() {
  createEffect(
    () => 0,
    () => {
      pickShow!();
    },
  );
  return (
    <p>
      <Shown />
    </p>
  );
}

// ... inside an in-place getter: an immediately invoked arrow in `Show`'s children.
let pickIife: (() => unknown) | undefined;

function ShownIife() {
  return (
    <Show when={1}>
      {(() => {
        pickIife = () => <Inner />;
        return <p>iife</p>;
      })()}
    </Show>
  );
}

export function ByShowIife() {
  createEffect(
    () => 0,
    () => {
      pickIife!();
    },
  );
  return (
    <p>
      <ShownIife />
    </p>
  );
}

// ... inside a `.map` callback in `Show`'s children.
let pickMap: (() => unknown) | undefined;

function ShownMap() {
  return (
    <Show when={1}>
      {[1].map(() => {
        pickMap = () => <Inner />;
        return <p>map</p>;
      })}
    </Show>
  );
}

export function ByShowMap() {
  createEffect(
    () => 0,
    () => {
      pickMap!();
    },
  );
  return (
    <p>
      <ShownMap />
    </p>
  );
}

// ... inside a context provider's children.
const Ctx = createContext(1);
let pickProvider: (() => unknown) | undefined;

function Provided() {
  return (
    <Ctx value={1}>
      {(() => {
        pickProvider = () => <Inner />;
        return <p>provider</p>;
      })()}
    </Ctx>
  );
}

export function ByProvider() {
  createEffect(
    () => 0,
    () => {
      pickProvider!();
    },
  );
  return (
    <p>
      <Provided />
    </p>
  );
}

// Control: the same functions, called where they are written (in place): Chrome is quiet and the checker has no row.
function Quiet() {
  return (
    <For each={[1]}>
      {(item) => {
        const view = () => <Inner />;
        return (
          <div>
            {view()}
            {item}
          </div>
        );
      }}
    </For>
  );
}

export function InPlaceControl() {
  return (
    <p>
      <Quiet />
    </p>
  );
}
