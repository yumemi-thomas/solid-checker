import { createMemo, createSignal, createTrackedEffect } from "solid-js";

// Proved V: the sealed holder's tag passes a signal read; the body reads it once.
function Card(props: { title: string }) {
  const title = props.title;
  return <p>{title}</p>;
}
// Proved clean: every tag passes a literal.
function Badge(props: { n: number }) {
  const n = props.n;
  return <p>{n}</p>;
}
// U, async-prop-getter: the tag passes a pending getter into a tracked effect.
function Panel(props: { value: number }) {
  createTrackedEffect(() => {
    props.value;
  });
  return <p />;
}
const Ui = { Card, Badge, Panel };

// U, prop-pending-unproven: the holder escapes, so no tag is resolved and
// the read in the tracked effect may read a pending getter.
function Loose(props: { value: number }) {
  createTrackedEffect(() => {
    props.value;
  });
  return <p />;
}
const Kit = { Loose };
export const kits = [Kit];

// Clean: a memo may read a pending value; only a leaf owner is asked.
function Derived(props: { value: number }) {
  const doubled = createMemo(() => props.value * 2);
  return <p>{doubled()}</p>;
}
const Box = { Derived };
export const boxes = [Box];

export function App() {
  const [label] = createSignal("a");
  const data = createMemo(async () => 1);
  return (
    <p>
      <Ui.Card title={label()} />
      <Ui.Badge n={1} />
      <Ui.Panel value={data()} />
      <Kit.Loose value={data()} />
      <Box.Derived value={data()} />
    </p>
  );
}
