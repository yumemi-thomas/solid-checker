import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <p data-view="v1">{(() => {
        const f1 = () => {
          const peek = (g: () => number) => g(); void peek(a1);;
        };
        f1();
        return 1;
      })()}</p>
    </div>
  );
}
