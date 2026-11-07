import { Errored, createMemo, createSignal, createTrackedEffect } from "solid-js";

export function App() {
  const f1 = () => {
    createMemo(async () => 1, { sync: true });
    return 1;
  };
  const K1 = () => { createTrackedEffect(() => { f1(); }); return <i></i>; };
  return (
    <div>
      <p data-view="v1"><Errored fallback={(e) => <i>{String(e())}</i>}><K1 /></Errored></p>
    </div>
  );
}
