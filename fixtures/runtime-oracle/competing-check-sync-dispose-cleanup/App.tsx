import { createEffect, createMemo, createRoot, createSignal } from "solid-js";

export function C1() {
  const disp1 = createRoot((dispose) => {
    createEffect(() => {
      return 1;
    }, (v) => {
      return () => {
        createMemo(async () => 1, { sync: true });
      };
    });
    return dispose;
  });
  return (
    <div>
      <button data-act="b1" onClick={() => {
        disp1();
      }}>go</button>
    </div>
  );
}
