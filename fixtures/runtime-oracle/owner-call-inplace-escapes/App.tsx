import { Errored, For, Match, Show, Switch, createEffect } from "solid-js";

// B2 (the re-review of #225, B1-B4): what a dialect control-flow element calls in place is the function in a position the DIALECT names (`For`'s row function, `Show`'s function
// child, `Errored`'s fallback), and only what is written directly in its body. Each export below is a function (or a class, or a `Match`) that the in-place rule as first built
// took for run in place, though it runs where it is called: stored, and called by an effect's apply, it creates a tag with no owner set, and Chrome rc.21 throws
// PRIMITIVE_IN_EFFECT_CALLBACK at the tag (createComponent runs its component under createRoot, which refuses an apply's stack with no owner).
// B1: a named function expression that stores itself (`ByNamedRow`, `ByNamedFallback`). B2: a function in `Show`'s `when`, which `Show` hands to its function child and
// never calls (`ByWhenKeyed`; `ByWhenValue`, the non-keyed twin, whose untracked read of the accessor is another rule's STRICT_READ_UNTRACKED). B3: JSX in an instance field's
// initializer, which runs at each construction (`ByClassField` in a component, `ByClassFieldInRow` in a function a dialect element calls). B4: a `Match` outside a `Switch`,
// which returns its props (`ByLooseMatch`). The controls at the end are quiet.

function Inner(props: { label?: string }) {
  return <p>{props.label ?? "inner"}</p>;
}

function Broken(): never {
  throw new Error("boom");
}

// B1 -------------------------------------------------------------------------------------------------------------------------------------------------------------------
let pickNamed: (() => unknown) | undefined;
function NamedRow() {
  return (
    <For each={[1]}>
      {function row() {
        pickNamed = row;
        return <Inner />;
      }}
    </For>
  );
}
export function ByNamedRow() {
  createEffect(
    () => 0,
    () => {
      pickNamed!();
    },
  );
  return (
    <p>
      <NamedRow />
    </p>
  );
}

let pickFallback: ((error: () => unknown, reset: () => void) => unknown) | undefined;
function NamedFallback() {
  return (
    <Errored
      fallback={function fb(_error: () => unknown, _reset: () => void) {
        pickFallback = fb;
        return <Inner />;
      }}
    >
      <Broken />
    </Errored>
  );
}
export function ByNamedFallback() {
  createEffect(
    () => 0,
    () => {
      pickFallback!(() => 0, () => {});
    },
  );
  return (
    <p>
      <NamedFallback />
    </p>
  );
}

// B2 -------------------------------------------------------------------------------------------------------------------------------------------------------------------
let pickWhenKeyed: (() => unknown) | undefined;
function WhenKeyed() {
  return (
    <Show
      keyed
      when={() => (
        <Inner />
      )}
    >
      {(view) => {
        pickWhenKeyed = view;
        return <p>when</p>;
      }}
    </Show>
  );
}
export function ByWhenKeyed() {
  createEffect(
    () => 0,
    () => {
      pickWhenKeyed!();
    },
  );
  return (
    <p>
      <WhenKeyed />
    </p>
  );
}

let pickWhenValue: (() => unknown) | undefined;
function WhenValue() {
  return (
    <Show
      when={() => (
        <Inner />
      )}
    >
      {(get) => {
        pickWhenValue = get();
        return <p>when</p>;
      }}
    </Show>
  );
}
export function ByWhenValue() {
  createEffect(
    () => 0,
    () => {
      pickWhenValue!();
    },
  );
  return (
    <p>
      <WhenValue />
    </p>
  );
}

// B3 -------------------------------------------------------------------------------------------------------------------------------------------------------------------
let pickClass: (new () => unknown) | undefined;
function MakesClass() {
  class Holder {
    view = (
      <Inner />
    );
  }
  pickClass = Holder;
  return <p>class</p>;
}
export function ByClassField() {
  createEffect(
    () => 0,
    () => {
      new pickClass!();
    },
  );
  return (
    <p>
      <MakesClass />
    </p>
  );
}

let pickRowClass: (new () => unknown) | undefined;
function MakesClassInRow() {
  return (
    <For each={[1]}>
      {() => {
        class Holder {
          view = (
            <Inner />
          );
        }
        pickRowClass = Holder;
        return <p>class in row</p>;
      }}
    </For>
  );
}
export function ByClassFieldInRow() {
  createEffect(
    () => 0,
    () => {
      new pickRowClass!();
    },
  );
  return (
    <p>
      <MakesClassInRow />
    </p>
  );
}

// B4 -------------------------------------------------------------------------------------------------------------------------------------------------------------------
function Tabs(props: { children?: unknown }) {
  createEffect(
    () => props.children,
    (match) => {
      if (match && typeof match === "object" && "children" in match) void match.children;
    },
  );
  return <p>tabs</p>;
}
export function ByLooseMatch() {
  return (
    <Tabs>
      <Match when={1}>
        <Inner />
      </Match>
    </Tabs>
  );
}

// Controls: quiet. A named function expression that never uses its name, `Match` under `Switch`, `Show`'s function child with its parameter.
function NamedQuiet() {
  return (
    <For each={[1]}>
      {function row() {
        return <Inner />;
      }}
    </For>
  );
}
function SwitchQuiet() {
  return (
    <Switch>
      <Match when={1}>
        <Inner />
      </Match>
    </Switch>
  );
}
function ShowQuiet() {
  return <Show when={1}>{(value) => <Inner label={String(value())} />}</Show>;
}
export function Controls() {
  return (
    <p>
      <NamedQuiet />
      <SwitchQuiet />
      <ShowQuiet />
    </p>
  );
}
