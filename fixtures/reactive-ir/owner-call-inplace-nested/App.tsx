import { For, Show, createContext, createEffect } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, B2 (the review of #225, the in-place rule; ADR 0971): a dialect control-flow element (`For`, `Show`, ...) calls the function it is handed, and reads its getters, under the
// owner of its own render, so a creation written DIRECTLY in that function's body or in the getter runs there and throws nowhere: no site. A function written INSIDE one of them is
// no part of that call: it runs where it is called, and here an effect's apply calls it with no owner set, where a tag is refused (rc.21 PRIMITIVE_IN_EFFECT_CALLBACK, SC6002).
// The rule as first built dropped every site inside the handed function or the getter, nested functions included, and certified these programs (Chrome:
// fixtures/runtime-oracle/owner-call-inplace-nested).

function Inner() {
  return <p>inner</p>;
}

// Controls: the tag is written directly in the function a dialect element is handed, or in its getter, or in a context provider's children.
function RowDirect() {
  return <For each={[1]}>{() => <Inner />}</For>;
}
function ShowFunctionDirect() {
  return <Show when={1}>{(_value) => <Inner />}</Show>;
}
function ShowGetterDirect() {
  return (
    <Show when={1}>
      <Inner />
    </Show>
  );
}
const Ctx = createContext(1);
function ProviderDirect() {
  return (
    <Ctx value={1}>
      <Inner />
    </Ctx>
  );
}

// A function written in the function handed to `For`, stored and called later by `Host`'s apply.
let pickRow: (() => unknown) | undefined;
function RowNested() {
  return (
    <For each={[1]}>
      {() => {
        const view = () => <Inner />;
        pickRow = view;
        return <p>row</p>;
      }}
    </For>
  );
}

// ... in the function child of `Show`.
let pickShow: (() => unknown) | undefined;
function ShowFunctionNested() {
  return (
    <Show when={1}>
      {(_value) => {
        const view = () => <Inner />;
        pickShow = view;
        return <p>show</p>;
      }}
    </Show>
  );
}

// ... in an immediately invoked arrow in `Show`'s children.
let pickIife: (() => unknown) | undefined;
function ShowGetterIife() {
  return (
    <Show when={1}>
      {(() => {
        pickIife = () => <Inner />;
        return <p>iife</p>;
      })()}
    </Show>
  );
}

// ... in a `.map` callback in `Show`'s children.
let pickMap: (() => unknown) | undefined;
function ShowGetterMap() {
  return (
    <Show when={1}>
      {[1].map(() => {
        pickMap = () => <Inner />;
        return <p>map</p>;
      })}
    </Show>
  );
}

// ... in an immediately invoked arrow in a context provider's children.
let pickProvider: (() => unknown) | undefined;
function ProviderIife() {
  return (
    <Ctx value={1}>
      {(() => {
        pickProvider = () => <Inner />;
        return <p>provider</p>;
      })()}
    </Ctx>
  );
}

// A closure made by a call to another function and kept by the function a dialect element is handed: the closure is written in `makeView`, outside the element.
function makeView() {
  return () => <Inner />;
}
let pickMade: (() => unknown) | undefined;
function RowMade() {
  return (
    <For each={[1]}>
      {() => {
        pickMade = makeView();
        return <p>made</p>;
      }}
    </For>
  );
}

function Host() {
  createEffect(
    () => 0,
    () => {
      pickRow!();
      pickShow!();
      pickIife!();
      pickMap!();
      pickProvider!();
      pickMade!();
    },
  );
  return <p>host</p>;
}

function App() {
  return (
    <p>
      <RowDirect />
      <ShowFunctionDirect />
      <ShowGetterDirect />
      <ProviderDirect />
      <RowNested />
      <ShowFunctionNested />
      <ShowGetterIife />
      <ShowGetterMap />
      <ProviderIife />
      <RowMade />
      <Host />
    </p>
  );
}
render(() => <App />, document.body);
