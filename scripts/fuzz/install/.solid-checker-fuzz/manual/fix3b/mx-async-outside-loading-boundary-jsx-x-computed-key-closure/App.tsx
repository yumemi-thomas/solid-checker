import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const { useIt } = { ["useIt"]: a1 };
  return (
    <div>
      <p data-view="v1">{(() => {
        const f1 = () => {
          useIt();
        };
        f1();
        return 1;
      })()}</p>
    </div>
  );
}
