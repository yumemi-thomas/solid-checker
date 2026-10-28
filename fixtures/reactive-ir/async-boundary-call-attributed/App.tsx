import { createMemo, For, isPending, Loading, Show } from "solid-js";
import { render } from "@solidjs/web";

// ADR 0462: an async memo read in a named local function renders its pending
// state where a call of that function sits in rendered JSX. Nothing proves the
// read runs while the call renders, so each call is uncertifiable, never a
// violation, even though the chain reaches `render()` with no boundary.

// Uncertifiable: a wrapper in the component body called from JSX.
function Wrapper() {
  const user = createMemo(async () => 1);
  const wrapUse = () => user();
  return <p>{wrapUse()}</p>;
}

// Uncertifiable: a closure called inside an IIFE the JSX runs.
function Closure() {
  const user = createMemo(async () => 1);
  return (
    <p>
      {(() => {
        const f1 = () => {
          user();
        };
        f1();
        return 1;
      })()}
    </p>
  );
}

// Uncertifiable: two named layers (wrapper called by a closure).
function TwoLayers() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return (
    <p>
      {(() => {
        const f1 = () => {
          wrapUse();
        };
        f1();
        return 1;
      })()}
    </p>
  );
}

// Uncertifiable: under `<Show>` and in a `<For>` row, which are not boundaries.
function UnderShowAndFor() {
  const user = createMemo(async () => 1);
  const wrapUse = () => user();
  return (
    <div>
      <Show when={true}>
        <p>{wrapUse()}</p>
      </Show>
      <For each={[1]}>{() => <p>{wrapUse()}</p>}</For>
    </div>
  );
}

// Violation (the direct read, unchanged): rendered with no boundary on a
// chain that reaches `render()`.
function Direct() {
  const user = createMemo(async () => 1);
  return <p>{user()}</p>;
}

// Negative: the wrapper's call is under `<Loading>`.
function UnderLoading() {
  const user = createMemo(async () => 1);
  const wrapUse = () => user();
  return (
    <Loading fallback={<p />}>
      <p>{wrapUse()}</p>
    </Loading>
  );
}

// Negative: the call is in an event handler, which does not render.
function InHandler() {
  const user = createMemo(async () => 1);
  const wrapUse = () => {
    user();
  };
  return <button onClick={() => wrapUse()}>go</button>;
}

// Negative: `isPending` observes the pending state.
function Probed() {
  const user = createMemo(async () => 1);
  const wrapUse = () => user();
  return <p>{isPending(() => wrapUse()) ? "…" : "ok"}</p>;
}

// Negative: a declared first paint never needs a boundary.
function Declared() {
  const user = createMemo(async () => 1, { loadingValue: 0 });
  const wrapUse = () => user();
  return <p>{wrapUse()}</p>;
}

// Negative: a synchronous memo is never pending.
function Sync() {
  const user = createMemo(() => 1);
  const wrapUse = () => user();
  return <p>{wrapUse()}</p>;
}

// Negative: a reassigned binding names no one function.
function Reassigned() {
  const user = createMemo(async () => 1);
  let wrapUse = () => user();
  wrapUse = () => 2;
  return <p>{wrapUse()}</p>;
}

function Page() {
  return (
    <div>
      <Wrapper />
      <Closure />
      <TwoLayers />
      <UnderShowAndFor />
      <Direct />
      <UnderLoading />
      <InHandler />
      <Probed />
      <Declared />
      <Sync />
      <Reassigned />
    </div>
  );
}

render(() => <Page />, document.body);
