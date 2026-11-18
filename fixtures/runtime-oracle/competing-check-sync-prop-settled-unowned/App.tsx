import { createMemo, createSignal, onSettled } from "solid-js";

export function C1() {
  return (
    <div>
      <Kid1 run={() => {
        createMemo(async () => 1, { sync: true });
      }} />
    </div>
  );
}
function Kid1(props: { run: () => void }) {
  return <button data-act="b1" onClick={() => { onSettled(() => { props.run(); }); }}>go</button>;
}
