import { createMemo, createSignal } from "solid-js";

export function C1() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <p data-view="v1">{(() => {
        const useM = (g: () => number) => createMemo(() => g()); let m = useM(a1); m = useM(a1); void m();;
        return 1;
      })()}</p>
    </div>
  );
}
