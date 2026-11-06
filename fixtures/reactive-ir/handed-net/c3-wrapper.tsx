import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const wrapUse = () => { a1(); };
  return (
    <div>
      <Kid1 run={() => {
        wrapUse();
      }} />
    </div>
  );
}

function Kid1(props: { run: () => void }) {
  return <p data-view="k1">{(props.run(), "x")}</p>;
}

export { C1 as App };
