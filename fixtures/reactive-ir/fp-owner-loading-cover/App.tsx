import { createMemo, createRenderEffect, createEffect, Loading, For } from "solid-js";
import { render } from "@solidjs/web";

const articles = createMemo(async () => {
  const response = await fetch("/api/articles");
  return (await response.json()) as { id: number }[];
});

// Negative: a memo that reads the async memo is not a render of it. The
// pending state flows to the render effect that consumes `filtered`, which is
// under <Loading>.
export function MemoChainUnderLoading() {
  const filtered = createMemo(() => articles().filter((article) => article.id !== 1));
  createEffect(
    () => articles().length,
    () => {},
  );
  return (
    <Loading fallback={<div />}>
      <For each={filtered()}>{(article) => <p>{article.id}</p>}</For>
    </Loading>
  );
}

// Negative: a memo whose value is an async FUNCTION is never pending; the
// async function is its value, not its computation.
export function AsyncClosureValue() {
  const run = createMemo(() => async () => {
    await fetch("/api/run");
  });
  return <div>{typeof run()}</div>;
}

// Negative: the leaf component is rendered by Middle, which is rendered under
// <Loading> two call-site hops above the read.
function Leaf() {
  return <div>{articles().length}</div>;
}

function Middle() {
  return <Leaf />;
}

export function TopWithLoading() {
  return (
    <Loading fallback={<div />}>
      <Middle />
    </Loading>
  );
}

// Uncertifiable: nothing in the project renders this component (a router
// hands it to the page outlet), so the boundary above it is not visible.
export function UnmountedRoute() {
  return <div>{articles().length}</div>;
}

// Positive control: every chain was followed to the `render` root and none
// passes through a boundary.
function Page() {
  return <div>{articles().length}</div>;
}

render(() => <Page />, document.body);

// Positive control: a render effect is a render of the pending read.
function RenderEffectPage() {
  createRenderEffect(
    () => articles().length,
    () => {},
  );
  return <div />;
}

render(() => <RenderEffectPage />, document.body);

// Negative (P-M): the async read is inside a callback prop. It runs when the
// row calls it, not while the JSX renders, so it is no render of the pending
// value, even mounted from a `render` root with no boundary.
function Row(props: { label: string; onPick: () => void }) {
  return <button onClick={() => props.onPick()}>{props.label}</button>;
}

function CallbackPropPage() {
  const pick = (_id: number | undefined) => {};
  return (
    <For each={["a"]}>
      {(label) => <Row label={label} onPick={() => pick(articles()[0]?.id)} />}
    </For>
  );
}

render(() => <CallbackPropPage />, document.body);
