import { createMemo, createSignal, createTrackedEffect, onSettled } from "solid-js";

// U, async-prop-getter: the only caller passes a pending getter, and the
// tracked effect destructures it.
function Pending(props: { v: number }) {
  createTrackedEffect(() => {
    const { v } = props;
    void v;
  });
  return <p />;
}
// U, async-prop-getter: the destructure runs in an IIFE inside a helper the
// tracked effect calls.
function Helper(props: { v: number }) {
  const read = () => {
    (() => {
      const { v } = props;
      return v;
    })();
  };
  createTrackedEffect(() => {
    read();
  });
  return <p />;
}
// Clean: a signal read in a tracked effect re-runs; no caller passes a
// pending getter.
function Settled(props: { v: number }) {
  createTrackedEffect(() => {
    const { v } = props;
    void v;
  });
  return <p />;
}
// U, prop-pending-unproven: exported, so an unseen caller may pass one.
export function Open(props: { v: number }) {
  onSettled(() => {
    const { v } = props;
    void v;
  });
  return <p />;
}

export function App() {
  const data = createMemo(async () => 1);
  const [count] = createSignal(1);
  return (
    <p>
      <Pending v={data()} />
      <Helper v={data()} />
      <Settled v={count()} />
    </p>
  );
}
