import { createEffect, createMemo, createSignal } from "solid-js";

export function C1() {
  return (
    <div>
      <Fwd1 run={() => {
        createMemo(async () => 1, { sync: true });
      }} />
    </div>
  );
}
function Kid1(props: { run: () => void }) {
  createEffect(() => 1, () => { props.run(); });
  return <p data-view="k1">x</p>;
}
function Fwd1(props: { run: () => void }) {
  props.run();
  return <Kid1 run={props.run} />;
}
