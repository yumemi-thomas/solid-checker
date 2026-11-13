import { Errored, For, Show, createMemo, createSignal, flush, onCleanup } from "solid-js";

// B2 (the review of #225, the coordinator's check of the in-place function rule): what Chrome rc.21 raises for the operations a function handed to a dialect control-flow element
// may hold, to show the rule keeps every row Chrome needs. `owner-call-inplace-function` is the legal form (a tag and a tracked expression in `Show`'s function child, `For`'s row
// function and `Errored`'s fallback: Chrome quiet). Here: `flush()` and a signal write in `For`'s row function, in `Show`'s function child and in `Errored`'s fallback, and
// `createMemo` and `onCleanup` in each of the three. One export each: an uncaught error halts the page before the next export runs.
//
// Measured (Chrome rc.21, on #220's head, on the build before the rule and on the build with it: the same three verdicts): the signal write raises REACTIVE_WRITE_IN_OWNED_SCOPE in all three
// (the row function and the function child also STRICT_READ_UNTRACKED at the `count()` read); `covered-violation` at both in `ForWrite` and `ShowWrite`, `covered-uncertifiable` at the
// write in `ErroredWrite`. `flush()`, `createMemo` and `onCleanup` raise nothing in any of the three. The rule changes no row here: a JSX creation is the only site it removes.

const [count, setCount] = createSignal(0);

function Broken(): never {
  throw new Error("boom");
}

export function ForFlush() {
  return (
    <For each={[1]}>
      {(item) => {
        flush();
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ForWrite() {
  return (
    <For each={[1]}>
      {(item) => {
        setCount(count() + 1);
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ForMemo() {
  return (
    <For each={[1]}>
      {(item) => {
        createMemo(() => item);
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ForCleanup() {
  return (
    <For each={[1]}>
      {(item) => {
        onCleanup(() => {});
        return <p>{item}</p>;
      }}
    </For>
  );
}

export function ShowFlush() {
  return (
    <Show when={1}>
      {(value) => {
        flush();
        return <p>{value()}</p>;
      }}
    </Show>
  );
}

export function ShowWrite() {
  return (
    <Show when={1}>
      {(value) => {
        setCount(count() + 1);
        return <p>{value()}</p>;
      }}
    </Show>
  );
}

export function ShowMemo() {
  return (
    <Show when={1}>
      {(value) => {
        createMemo(() => value());
        return <p>{value()}</p>;
      }}
    </Show>
  );
}

export function ShowCleanup() {
  return (
    <Show when={1}>
      {(value) => {
        onCleanup(() => {});
        return <p>{value()}</p>;
      }}
    </Show>
  );
}

export function ErroredMemo() {
  return (
    <Errored
      fallback={(error) => {
        createMemo(() => String(error()));
        return <p>fallback</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

export function ErroredCleanup() {
  return (
    <Errored
      fallback={(error) => {
        onCleanup(() => {});
        return <p>{String(error())}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

export function ErroredWrite() {
  return (
    <Errored
      fallback={(error) => {
        setCount(count() + 1);
        return <p>{String(error())}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}

export function ErroredFlush() {
  return (
    <Errored
      fallback={(error) => {
        flush();
        return <p>{String(error())}</p>;
      }}
    >
      <Broken />
    </Errored>
  );
}
