import { createEffect, createRoot, createSignal } from "solid-js";

const probe = { disposedRuns: 0, emptyRuns: 0 };
Object.assign(globalThis, { rootDisposalProbe: probe });

export function DisposedRoot() {
  const [count, setCount] = createSignal(0);
  return <section>
    <button data-probe="disposed-write" onClick={() => setCount(1)}>write</button>
    <div ref={() => {
      createRoot(dispose => {
        createEffect(() => count(), () => { probe.disposedRuns++; });
        dispose();
      });
    }} />
  </section>;
}

export function EmptyRoot() {
  return <div ref={() => {
    createRoot(() => { probe.emptyRuns++; });
  }} />;
}
