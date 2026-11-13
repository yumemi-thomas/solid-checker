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

// The follow-ups of the re-check of #225 (N2), appended so the sites above keep their spans.
// B3, the `accessor` form: an auto-accessor field is an own property of every instance as a plain field is, and its initializer runs at each construction.
let pickAccessorClass: (new () => unknown) | undefined;
function MakesAccessorClass() {
  class Holder {
    accessor view = (
      <Inner />
    );
  }
  pickAccessorClass = Holder;
  return <p>accessor class</p>;
}

// B2, a default or a rest parameter: `Show` calls its function child only when the child declares a parameter it counts (`Function.length`: `child.length > 0` in rc.21's `solid.dev.js`).
// A default parameter and a rest parameter count for none, so `Show` returns the child itself as its value, a function whoever reads the `Show` calls: `Late`, on an effect's apply, with no
// owner set. Both forms are tsc-clean on the real typings: `(v: () => number = dflt) =>` and `(...v: (() => number)[]) =>` pass `NonZeroParams` (the length of their `Parameters` is `0 | 1`
// and `number`, never exactly `0`) and their `Function.length` is 0; the zero-parameter form is the one tsc rejects. A site in either body is a site like any other.
const dflt = () => 1;
function Late(props: { children: unknown }) {
  // The compute reads the children, so `Show` is created under the compute's owner; the apply calls what `Show` returned, and what that returns, with no owner set.
  createEffect(
    () => props.children,
    (shown) => {
      if (typeof shown === "function") {
        const view: unknown = shown();
        if (typeof view === "function") view();
      }
    },
  );
  return <p>late</p>;
}
function ShowDefaultParam() {
  return (
    <Late>
      <Show when={1}>
        {(_value: () => number = dflt) => (
          <Inner />
        )}
      </Show>
    </Late>
  );
}
function ShowRestParam() {
  return (
    <Late>
      <Show when={1}>
        {(..._values: (() => number)[]) => (
          <Inner />
        )}
      </Show>
    </Late>
  );
}

// B1, what reading `arguments` does (`can_hand_itself_on`): it sends an inline function expression to the entry walk as a site like any other instead of taking it for one the element calls in
// place, though `arguments.callee` throws in module code and nothing else of `arguments` is the function. It moves no row where the walk closes the function: `For`'s function child is a
// control-flow child no reference names, so it is a site that ends in a silence. `Errored`'s `fallback` is no control-flow child, so the walk does not close it and the site keeps its row.
function ForReadsArguments() {
  return (
    <For each={[1]}>
      {function () {
        void arguments.length;
        return <Inner />;
      }}
    </For>
  );
}
function ErroredReadsArguments() {
  return (
    <Errored
      fallback={function () {
        void arguments.length;
        return <Inner />;
      }}
    >
      <Broken />
    </Errored>
  );
}
// The control of the two above: the same `fallback` that never reads `arguments` is called in place, and its tag is no site.
function ErroredQuiet() {
  return (
    <Errored
      fallback={function () {
        return <Inner />;
      }}
    >
      <Broken />
    </Errored>
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
// The control of `ShowDefaultParam` and `ShowRestParam`, read late by the same `Late`: with a parameter `Show` calls the child itself, in place, and `Late` finds its result, not a function.
function ShowParamLate() {
  return (
    <Late>
      <Show when={1}>
        {(_value: () => number) => (
          <Inner />
        )}
      </Show>
    </Late>
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
      new pickAccessorClass!();
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
      <MakesAccessorClass />
      <ShowDefaultParam />
      <ShowRestParam />
      <ForReadsArguments />
      <ErroredReadsArguments />
      <ErroredQuiet />
      <LooseMatch />
      <NamedQuiet />
      <SwitchQuiet />
      <ShowQuiet />
      <ShowWithComment />
      <ShowParamLate />
      <Host />
    </p>
  );
}
render(() => <App />, document.body);
