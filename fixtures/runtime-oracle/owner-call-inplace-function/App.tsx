import { Errored, For, Match, Repeat, Show, Switch } from "solid-js";

// B2 (the review of #225, `make repro-causes`: `clean-errored-reset`): a function handed to a dialect control-flow element is called by that element's own render, under the owner
// it sets, so a creation in it throws nowhere and the tag is no owner-call site. `Show`'s function child, `For`'s row function and `Errored`'s fallback each create a component
// tag and a tracked expression. Chrome rc.21 is quiet on all three; the checker has no row at them.

function Inner(props: { label: string }) {
  return <p>{props.label}</p>;
}

function Broken(): never {
  throw new Error("boom");
}

export function ShowFunction() {
  return <Show when={1}>{(value) => <Inner label={String(value())} />}</Show>;
}

export function ForRows() {
  return <For each={[1, 2]}>{(item) => <Inner label={String(item)} />}</For>;
}

export function ErroredFallback() {
  return (
    <Errored fallback={(error, reset) => <Inner label={String(error()) + typeof reset} />}>
      <Broken />
    </Errored>
  );
}

// The other function forms the dialect's control-flow elements take: `Show`'s keyed function child, `For`'s non-keyed row function, `Repeat`'s index function and `Match`'s function child.
export function ShowKeyed() {
  return (
    <Show when={1} keyed>
      {(value) => <Inner label={String(value)} />}
    </Show>
  );
}

export function ForNotKeyed() {
  return (
    <For each={[1, 2]} keyed={false}>
      {(item, index) => <Inner label={String(item()) + String(index)} />}
    </For>
  );
}

export function RepeatRows() {
  return <Repeat count={2}>{(index) => <Inner label={String(index)} />}</Repeat>;
}

export function MatchFunction() {
  return (
    <Switch>
      <Match when={1}>{(value) => <Inner label={String(value())} />}</Match>
    </Switch>
  );
}
