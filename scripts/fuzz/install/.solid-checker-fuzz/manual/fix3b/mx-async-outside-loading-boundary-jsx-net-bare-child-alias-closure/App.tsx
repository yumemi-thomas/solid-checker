import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  const useIt = a1;
  return (
    <div>
      <p data-view="v1">{(() => {
        const f1 = () => {
          void (<b>{useIt as unknown as string}</b>);;
        };
        f1();
        return 1;
      })()}</p>
    </div>
  );
}
