import { Errored, For, Show } from "solid-js";

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
