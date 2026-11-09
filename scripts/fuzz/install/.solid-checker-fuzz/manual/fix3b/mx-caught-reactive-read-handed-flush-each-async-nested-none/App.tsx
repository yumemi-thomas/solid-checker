import { createMemo, createSignal, flush } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const { a: { useIt } } = { a: { useIt: a1 } };
  const K1 = () => {
    flush(() => {
      try { useIt(); } catch { };
    });
    return <span />;
  };
  return (
    <div>
      <p data-view="v1">{(() => {
        [1].forEach(K1);
        return 1;
      })()}</p>
    </div>
  );
}
