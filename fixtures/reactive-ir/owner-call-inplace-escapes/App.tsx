import { Errored, For, Match, Show, Switch, createEffect } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, B2 (the re-review of #225, B1-B4; ADR 0971): what a dialect control-flow element calls in place is the function in a position the DIALECT names (`For`'s row function,
// `Show`'s function child with a parameter, `Errored`'s fallback), and only what is written directly in its body. Each site below is a tag the in-place rule as first built took for
// created in place, though it is created where something else runs it (stored and called by `Host`'s effect apply, with no owner set: rc.21 PRIMITIVE_IN_EFFECT_CALLBACK, SC6002).
// Chrome: fixtures/runtime-oracle/owner-call-inplace-escapes, `unproven-clean` on that build for every one of them, `covered-uncertifiable` now.

function Inner() {
  return <p>inner</p>;
}

function Broken(): never {
  throw new Error("boom");
}

// B1: a named function expression that stores itself runs where it is called.
let pickNamed: (() => unknown) | undefined;
function NamedSelf() {
  return (
    <For each={[1]}>
      {function row() {
        pickNamed = row;
        return <Inner />;
      }}
    </For>
  );
}
let pickFallback: ((error: () => unknown, reset: () => void) => unknown) | undefined;
function NamedFallbackSelf() {
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

// B2: `Show` hands its `when` value to the function child and never calls it.
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
// B3: JSX in an instance field's initializer runs at each construction, which may be on any stack: in a component, and in a function a dialect element calls.
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

// B4: a `Match` outside a `Switch` returns its props: whoever reads them runs the getter.
function Tabs(props: { children?: unknown }) {
  createEffect(
    () => props.children,
    (match) => {
      if (match && typeof match === "object" && "children" in match) void match.children;
    },
  );
  return <p>tabs</p>;
}
function LooseMatch() {
  return (
    <Tabs>
      <Match when={1}>
        <Inner />
      </Match>
    </Tabs>
  );
}

// Controls (no row): a named function expression that never uses its name, `Match` as `Switch`'s direct child, `Show`'s function child with its parameter.
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
  return <Show when={1}>{(_value) => <Inner />}</Show>;
}
// A comment between the tags is no child at run time (the compiler drops it): the function is still the only child `Show` calls.
function ShowWithComment() {
  return (
    <Show when={1}>
      {/* a note */}
      {(_value) => <Inner />}
    </Show>
  );
}

function Host() {
  createEffect(
    () => 0,
    () => {
      pickNamed!();
      pickFallback!(() => 0, () => {});
      pickWhenKeyed!();
      new pickClass!();
      new pickRowClass!();
    },
  );
  return <p>host</p>;
}

function App() {
  return (
    <p>
      <NamedSelf />
      <NamedFallbackSelf />
      <WhenKeyed />
      <MakesClass />
      <MakesClassInRow />
      <LooseMatch />
      <NamedQuiet />
      <SwitchQuiet />
      <ShowQuiet />
      <ShowWithComment />
      <Host />
    </p>
  );
}
render(() => <App />, document.body);
