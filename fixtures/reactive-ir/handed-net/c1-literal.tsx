import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const box = { useIt: a1 };
  return (
    <div>
      <p data-view="v1">{(() => {
        box.useIt();
        return 1;
      })()}</p>
    </div>
  );
}

export { C1 as App };
