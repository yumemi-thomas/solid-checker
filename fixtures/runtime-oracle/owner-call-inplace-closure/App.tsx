import { For, Show, createEffect } from "solid-js";

// B2 (the review of #225, the in-place rule; the coordinator's probe of a closure made by a call): the function handed to `For` (or `Show`) calls another function that RETURNS a
// closure and keeps the result. The closure is written in `makeView`, outside the element, so the rule never sees it; what places it is the usual analysis of a function value
// that leaves the function that returns it. Chrome rc.21 throws PRIMITIVE_IN_EFFECT_CALLBACK at the `<Inner />` in the closure when an effect's apply calls it. `Rows` and `Shown`
// are written outside the exported components, which only a tag renders, so B3 (ADR 0967) keeps no row at their sites.

function Inner(props: { label?: string }) {
  return <p>{props.label ?? "inner"}</p>;
}

function makeView() {
  return () => <Inner />;
}

let pickRow: (() => unknown) | undefined;

function Rows() {
  return (
    <For each={[1]}>
      {(item) => {
        pickRow = makeView();
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ByRowMade() {
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

let pickShow: (() => unknown) | undefined;

function Shown() {
  return (
    <Show when={1}>
      {(value) => {
        pickShow = makeView();
        return <p>{String(value())}</p>;
      }}
    </Show>
  );
}

export function ByShowMade() {
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

// The call in an in-place getter (no function between): `makeView()` runs in place, its closure does not.
let pickGetter: (() => unknown) | undefined;

function ShownGetter() {
  return <Show when={1}>{(pickGetter = makeView(), (<p>getter</p>))}</Show>;
}

export function ByShowGetterMade() {
  createEffect(
    () => 0,
    () => {
      pickGetter!();
    },
  );
  return (
    <p>
      <ShownGetter />
    </p>
  );
}

// A helper that takes the row's data and returns a closure over it, stored in a module variable.
function makeViewFor(label: string) {
  return () => <Inner label={label} />;
}

let pickFor: (() => unknown) | undefined;

function RowsFor() {
  return (
    <For each={["a"]}>
      {(item) => {
        pickFor = makeViewFor(item);
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ByRowMadeFor() {
  createEffect(
    () => 0,
    () => {
      pickFor!();
    },
  );
  return (
    <p>
      <RowsFor />
    </p>
  );
}

// Control: the closure made by the call is called where the row function runs, in place: Chrome is quiet.
function RowsReturned() {
  return <For each={[1]}>{() => makeView()()}</For>;
}

export function ReturnedControl() {
  return (
    <p>
      <RowsReturned />
    </p>
  );
}
