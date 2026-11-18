import { createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  function run1() {
    createMemo(async () => 1, { sync: true });
  }
  return (
    <div>
      <Kid1 run={run1} />
    </div>
  );
}
function Kid1(props: { run: () => void }) {
  onSettled(() => { props.run(); });
  return <p data-view="k1">x</p>;
}
